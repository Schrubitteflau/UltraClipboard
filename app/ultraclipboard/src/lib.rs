//! Service D-Bus d'UltraClipboard, sans fenêtre : utilisé par l'hôte Tauri
//! (`app/desktop`, binaire `ultraclipboard`) et par les tests de contrat.
//!
//! L'extension GNOME Shell appelle `Copied(text)` à chaque copie de texte, et
//! `CopiedNonText()` à chaque copie sans texte (image, fichiers…) ; quand un
//! résultat est prêt, le service émet le signal `Apply(text)` et l'extension
//! l'écrit dans le presse-papier. Toute copie, texte ou non, arrête le
//! traitement de la précédente.

mod config;

use std::fmt;

use engine::{Engine, Outcome};
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::{Connection, interface, object_server::SignalEmitter};

pub use config::{ConfigFollower, config_path, follow_config, start_engine};

pub const BUS_NAME: &str = "org.ultraclipboard.UltraClipboard";
pub const OBJECT_PATH: &str = "/org/ultraclipboard/UltraClipboard";
pub const INTERFACE: &str = "org.ultraclipboard.UltraClipboard1";

/// Pourquoi le service n'a pas pu démarrer.
#[derive(Debug)]
pub enum ServeError {
    /// Une autre instance tient déjà le nom sur le bus.
    AlreadyRunning,
    Bus(zbus::Error),
}

impl fmt::Display for ServeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning => write!(f, "déjà lancé ({BUS_NAME} est pris)"),
            Self::Bus(err) => write!(f, "bus D-Bus : {err}"),
        }
    }
}

impl std::error::Error for ServeError {}

impl From<zbus::Error> for ServeError {
    fn from(err: zbus::Error) -> Self {
        Self::Bus(err)
    }
}

/// Expose le moteur sur `conn` et prend le nom du service.
///
/// Rend la main une fois le service en place ; il tourne tant que `conn` vit.
/// Une seule instance : si le nom est déjà pris, renvoie [`ServeError::AlreadyRunning`]
/// au lieu d'attendre en file.
pub async fn serve(conn: &Connection, engine: Engine) -> Result<(), ServeError> {
    conn.object_server()
        .at(OBJECT_PATH, Service { engine })
        .await?;
    match conn
        .request_name_with_flags(BUS_NAME, RequestNameFlags::DO_NOT_QUEUE)
        .await
    {
        Ok(RequestNameReply::PrimaryOwner) => Ok(()),
        Ok(_) | Err(zbus::Error::NameTaken) => Err(ServeError::AlreadyRunning),
        Err(err) => Err(err.into()),
    }
}

struct Service {
    engine: Engine,
}

// `spawn = false` : zbus traite les appels un à un, dans l'ordre d'arrivée sur le
// bus, au lieu d'une tâche par appel qui pourrait réordonner une rafale de copies.
// Sans risque de blocage : `copied` ne fait que lancer le traitement en tâche de fond.
#[interface(name = "org.ultraclipboard.UltraClipboard1", spawn = false)]
impl Service {
    /// Signale une copie. Rend la main aussitôt : le traitement part en tâche de
    /// fond et son résultat arrive par `Apply`.
    fn copied(&self, text: String, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let work = self.engine.on_copy(&text);
        let emitter = emitter.into_owned();
        tokio::spawn(async move {
            match work.await {
                Outcome::Write(output) => {
                    if let Err(err) = Self::apply(&emitter, &output).await {
                        eprintln!("ultraclipboard: échec d'émission de Apply : {err}");
                    }
                }
                Outcome::Failed(failure) => {
                    let mut line = format!(
                        "ultraclipboard: échec de l'étape {} : {}",
                        failure.step, failure.message
                    );
                    let stderr = failure.stderr.trim_end_matches('\n');
                    if !stderr.is_empty() {
                        line = format!("{line}\n{stderr}");
                    }
                    eprintln!("{line}");
                }
                Outcome::Nothing(_) | Outcome::Cancelled => {}
            }
        });
    }

    /// Signale une copie sans texte (image, fichiers…) : rien à transformer,
    /// mais le traitement d'une copie précédente s'arrête et son résultat ne
    /// remplace jamais ce qui vient d'être copié.
    fn copied_non_text(&self) {
        self.engine.on_copy_without_text();
    }

    /// Demande à l'extension d'écrire ce texte dans le presse-papier.
    #[zbus(signal)]
    async fn apply(emitter: &SignalEmitter<'_>, text: &str) -> zbus::Result<()>;
}
