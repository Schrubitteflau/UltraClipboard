//! `command` : lance un exécutable de l'utilisateur (ADR 0002).
//!
//! Le texte part sur l'entrée standard, le résultat est la sortie standard.

use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command as Process;

use super::{BuildError, StepError, StepFuture, Transformation};
use crate::config::yaml::Value;

/// Délai d'une étape sans `timeout`.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct Command {
    /// Programme puis arguments, `~/` déjà développé.
    argv: Vec<String>,
    timeout: Duration,
}

pub(super) fn build(params: &Value, timeout: Option<Duration>) -> Result<Command, BuildError> {
    let items = match params {
        Value::Seq(items) if items.is_empty() => {
            return Err(BuildError::params(
                "",
                "liste vide : au moins le programme à lancer",
            ));
        }
        Value::Seq(items) => items,
        other => {
            return Err(BuildError::params(
                "",
                format!(
                    "attendu une liste d'arguments (le programme puis ses arguments), trouvé {}",
                    other.kind()
                ),
            ));
        }
    };
    let home = std::env::home_dir();
    let argv = items
        .iter()
        .enumerate()
        .map(|(i, item)| match item {
            Value::Str(arg) => Ok(expand_home(arg, home.as_deref())),
            Value::Int(_) | Value::Float(_) => Err(BuildError::params(
                format!("[{i}]"),
                format!(
                    "attendu un texte, trouvé {} (mettre entre guillemets)",
                    item.kind()
                ),
            )),
            other => Err(BuildError::params(
                format!("[{i}]"),
                format!(
                    "attendu un texte, trouvé {}",
                    other.kind_where_text_expected()
                ),
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Command {
        argv,
        timeout: timeout.unwrap_or(DEFAULT_TIMEOUT),
    })
}

/// `~/` en début d'argument désigne le dossier personnel ; rien d'autre n'est
/// développé (ni `~` seul, ni `~nom/`, ni `~/` plus loin dans l'argument).
fn expand_home(arg: &str, home: Option<&Path>) -> String {
    match (arg.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => home.join(rest).to_string_lossy().into_owned(),
        _ => arg.to_owned(),
    }
}

impl Transformation for Command {
    fn name(&self) -> &str {
        "command"
    }

    fn apply<'a>(&'a self, input: &'a str) -> StepFuture<'a> {
        Box::pin(self.run(input))
    }
}

impl Command {
    async fn run(&self, input: &str) -> Result<String, StepError> {
        let mut child = Process::new(&self.argv[0])
            .args(&self.argv[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Son propre groupe : au délai ou à l'annulation, on tue aussi les
            // processus qu'il a lancés, qui sinon garderaient ses sorties ouvertes.
            .process_group(0)
            .spawn()
            .map_err(|err| {
                let message = format!("impossible de lancer « {} » : {err}", self.argv[0]);
                failure(message, Capped::default())
            })?;
        let mut group = Group(child.id());
        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (mut out, mut err) = (Vec::new(), Capped::default());
        let mut exited = None;
        // L'entrée est écrite pendant qu'on lit les sorties : un gros texte ne
        // bloque pas le programme qui écrit déjà sa sortie. Un programme qui
        // n'a pas lu son entrée n'est pas une erreur.
        let write = async move {
            let _ = stdin.write_all(input.as_bytes()).await;
            Ok(())
        };
        // Comme `$(…)`, on attend la fin du programme et la fermeture de ses
        // sorties, y compris par les processus qu'il a lancés.
        let finished = tokio::time::timeout(self.timeout, async {
            tokio::try_join!(
                write,
                drain_stdout(stdout, &mut out),
                drain_stderr(stderr, &mut err),
                async {
                    exited = Some(child.wait().await);
                    Ok(())
                },
            )
        })
        .await;
        let status = match (finished, exited) {
            (Ok(Ok(_)), Some(status)) => {
                group.0 = None;
                match status {
                    Ok(status) => status,
                    Err(e) => return Err(failure(format!("{e}"), err)),
                }
            }
            (Ok(Err(TooLarge)), _) => {
                group.kill();
                let _ = child.wait().await;
                let message = format!(
                    "sortie standard trop grande (plus de {} Mo)",
                    STDOUT_CAP / 1_000_000
                );
                return Err(failure(message, err));
            }
            (_, exited) => {
                group.kill();
                let _ = child.wait().await;
                let mut message = format!("délai dépassé ({} s)", self.timeout.as_secs_f64());
                if exited.is_some() {
                    message.push_str(
                        " : le programme a fini, mais un processus lancé en arrière-plan \
                         garde sa sortie ouverte",
                    );
                }
                return Err(failure(message, err));
            }
        };
        if !status.success() {
            return Err(failure(describe(status), err));
        }
        let mut out =
            String::from_utf8(out).map_err(|_| failure("sortie standard non UTF-8".into(), err))?;
        out.truncate(out.trim_end_matches('\n').len());
        Ok(out)
    }
}

/// Groupe de processus d'une commande en cours, tué s'il est lâché avant la
/// fin : quand le Pipeline est abandonné, la commande ne lui survit pas.
struct Group(Option<u32>);

impl Group {
    fn kill(&mut self) {
        if let Some(pgid) = self.0.take() {
            // SAFETY: simple appel système, sans mémoire partagée.
            unsafe {
                libc::killpg(pgid as libc::pid_t, libc::SIGKILL);
            }
        }
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Au-delà, la sortie standard fait échouer l'étape : ce n'est plus du texte
/// à coller, et la mémoire reste bornée.
const STDOUT_CAP: usize = 8_000_000;

/// Au-delà, la sortie d'erreur est coupée (et continue d'être lue, pour ne pas
/// bloquer le programme).
const STDERR_CAP: usize = 64_000;

/// La sortie standard dépasse [`STDOUT_CAP`].
struct TooLarge;

/// Lit la sortie standard jusqu'au bout dans `buf`. Annulé, `buf` garde ce qui
/// a été lu.
async fn drain_stdout(
    mut reader: impl AsyncRead + Unpin,
    buf: &mut Vec<u8>,
) -> Result<(), TooLarge> {
    let mut chunk = [0; 8192];
    while let Ok(n @ 1..) = reader.read(&mut chunk).await {
        if buf.len() + n > STDOUT_CAP {
            return Err(TooLarge);
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(())
}

/// Sortie d'erreur gardée, coupée à [`STDERR_CAP`].
#[derive(Default)]
struct Capped {
    kept: Vec<u8>,
    cut: bool,
}

/// Lit la sortie d'erreur jusqu'au bout dans `buf`. Annulé, `buf` garde ce qui
/// a été lu.
async fn drain_stderr(
    mut reader: impl AsyncRead + Unpin,
    buf: &mut Capped,
) -> Result<(), TooLarge> {
    let mut chunk = [0; 8192];
    while let Ok(n @ 1..) = reader.read(&mut chunk).await {
        let room = STDERR_CAP - buf.kept.len();
        buf.kept.extend_from_slice(&chunk[..n.min(room)]);
        buf.cut |= n > room;
    }
    Ok(())
}

fn describe(status: ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt;
    match (status.code(), status.signal()) {
        (Some(code), _) => format!("code de sortie {code}"),
        (None, Some(signal)) => format!("tué par le signal {signal}"),
        (None, None) => format!("{status}"),
    }
}

fn failure(message: String, stderr: Capped) -> StepError {
    let mut text = String::from_utf8_lossy(&stderr.kept).into_owned();
    if stderr.cut {
        text.push_str(&format!(
            "\n[… sortie d'erreur coupée après {} Ko]\n",
            STDERR_CAP / 1000
        ));
    }
    StepError {
        message,
        stderr: text,
    }
}
