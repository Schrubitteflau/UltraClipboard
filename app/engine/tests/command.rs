//! Transformation `command`, avec de vrais exécutables du système.

use std::time::{Duration, Instant};

mod common;

use common::{pids, pids_written_to, scratch, wait_dead};
use engine::{Config, ConfigError, Engine, Failure, Outcome, SAMPLE_CONFIG, Skip};

/// Moteur dont l'unique Règle applique à toute copie le Pipeline `steps`
/// (étapes YAML en style bloc, une par ligne, indentées de quatre espaces).
fn engine_with(steps: &str) -> Engine {
    let yaml = format!(
        "config_version: 1\npipelines:\n  p:\n{steps}\nrules:\n  - match: ''\n    pipeline: p\n"
    );
    Engine::new(Config::parse(&yaml).unwrap())
}

fn write(text: &str) -> Outcome {
    Outcome::Write(text.into())
}

#[tokio::test]
async fn the_text_goes_in_on_stdin_and_the_result_comes_from_stdout() {
    let engine = engine_with("    - command: [sh, -c, 'printf \"<%s>\" \"$(cat)\"']");
    assert_eq!(engine.on_copy("abc").await, write("<abc>"));
}

#[tokio::test]
async fn the_sample_config_strips_trackers_from_a_copied_link() {
    let engine = Engine::new(Config::parse(SAMPLE_CONFIG).unwrap());
    for (copied, cleaned) in [
        (
            "https://exemple.fr/article?utm_source=lettre&fbclid=abc",
            "https://exemple.fr/article",
        ),
        (
            " https://exemple.fr/p?id=3&utm_medium=mail&page=2#titre\n",
            "https://exemple.fr/p?id=3&page=2#titre",
        ),
        (
            "https://exemple.fr/p?gclid=1&id=3",
            "https://exemple.fr/p?id=3",
        ),
    ] {
        assert_eq!(engine.on_copy(copied).await, write(cleaned), "{copied:?}");
    }
    // Un lien sans traqueur, ou un texte qui n'est pas un lien, n'est pas touché.
    for untouched in [
        "https://exemple.fr/autre?id=4",
        "voir https://exemple.fr/p?utm_source=lettre demain",
    ] {
        assert_eq!(
            engine.on_copy(untouched).await,
            Outcome::Nothing(Skip::NoRule),
            "{untouched:?}"
        );
    }
}

#[tokio::test]
async fn trailing_newlines_are_removed_like_shell_substitution() {
    // Seuls les retours à la ligne finaux partent ; le reste est gardé tel quel.
    let engine = engine_with("    - command: [printf, ' a\\n\\nb \\n\\n\\n']");
    assert_eq!(engine.on_copy("x").await, write(" a\n\nb "));
}

#[tokio::test]
async fn a_non_zero_exit_fails_and_keeps_the_error_output() {
    let engine =
        engine_with("    - command: [sh, -c, 'echo partiel; echo \"ça casse\" >&2; exit 3']");
    assert_eq!(
        engine.on_copy("abc").await,
        Outcome::Failed(Failure {
            step: "command".into(),
            message: "code de sortie 3".into(),
            stderr: "ça casse\n".into(),
        })
    );
}

#[tokio::test]
async fn false_fails_with_no_error_output() {
    let engine = engine_with("    - command: ['false']");
    assert_eq!(
        engine.on_copy("abc").await,
        Outcome::Failed(Failure {
            step: "command".into(),
            message: "code de sortie 1".into(),
            stderr: String::new(),
        })
    );
}

#[tokio::test]
async fn past_its_timeout_the_command_fails_and_is_killed_with_its_children() {
    // `sh` lance `sleep` en enfant et l'attend : les deux doivent être tués.
    let engine = engine_with(
        "    - command: [sh, -c, 'echo $$ >&2; sleep 30 & echo $! >&2; wait']\n      timeout: 0.3",
    );
    let started = Instant::now();
    let Outcome::Failed(failure) = engine.on_copy("abc").await else {
        panic!("attendu un échec");
    };
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(failure.step, "command");
    assert_eq!(failure.message, "délai dépassé (0.3 s)");
    // La sortie d'erreur écrite avant le délai est gardée.
    let pids = pids(&failure.stderr);
    assert_eq!(pids.len(), 2, "sortie d'erreur : {:?}", failure.stderr);
    for pid in pids {
        assert!(wait_dead(pid).await, "le processus {pid} vit encore");
    }
}

#[tokio::test]
async fn an_abandoned_copy_kills_its_command() {
    let pidfile = scratch("abandoned.pid");
    let engine = engine_with(&format!(
        "    - command: [sh, -c, 'echo $$ > \"$0\"; exec sleep 30', '{}']",
        pidfile.display()
    ));
    let copy = tokio::spawn(engine.on_copy("abc"));
    let [pid] = pids_written_to(&pidfile, 1).await[..] else {
        unreachable!()
    };
    copy.abort();
    let _ = copy.await;
    assert!(wait_dead(pid).await, "la commande abandonnée vit encore");
    std::fs::remove_file(&pidfile).unwrap();
}

#[tokio::test]
async fn a_failing_step_stops_the_pipeline_and_leaves_the_clipboard_alone() {
    let marker = scratch("second-step-ran");
    let engine = engine_with(&format!(
        "    - upper\n    - command: [sh, -c, 'exit 1']\n    - command: [touch, '{}']",
        marker.display()
    ));
    let Outcome::Failed(failure) = engine.on_copy("abc").await else {
        panic!("attendu un échec");
    };
    assert_eq!(failure.message, "code de sortie 1");
    assert!(!marker.exists(), "l'étape après l'échec a tourné");
}

#[tokio::test]
async fn a_leading_tilde_slash_is_the_home_directory() {
    let home = std::env::var("HOME").unwrap();
    let engine = engine_with(
        "    - command: [sh, -c, 'printf \"%s|\" \"$@\"', sh, '~/notes', 'a~/b', '~', '~x/y']",
    );
    assert_eq!(
        engine.on_copy("abc").await,
        write(&format!("{home}/notes|a~/b|~|~x/y|"))
    );
}

#[tokio::test]
async fn a_large_text_goes_through_without_blocking() {
    let text = "é123456789\n".repeat(50_000) + "fin";
    let engine = engine_with("    - command: [sh, -c, 'cat; echo !']");
    assert_eq!(engine.on_copy(&text).await, write(&format!("{text}!")));
}

#[tokio::test]
async fn a_program_that_cannot_start_fails() {
    let engine = engine_with("    - command: [ultraclipboard-no-such-program]");
    let Outcome::Failed(failure) = engine.on_copy("abc").await else {
        panic!("attendu un échec");
    };
    assert!(
        failure
            .message
            .starts_with("impossible de lancer « ultraclipboard-no-such-program » : "),
        "{}",
        failure.message
    );
}

#[tokio::test]
async fn an_output_that_is_not_utf8_fails() {
    let engine = engine_with("    - command: [printf, '\\377']");
    assert_eq!(
        engine.on_copy("abc").await,
        Outcome::Failed(Failure {
            step: "command".into(),
            message: "sortie standard non UTF-8".into(),
            stderr: String::new(),
        })
    );
}

fn config_error(steps: &str) -> ConfigError {
    let yaml = format!("config_version: 1\npipelines:\n  p:\n{steps}\n");
    Config::parse(&yaml).err().expect("config acceptée")
}

#[test]
fn the_command_must_be_a_non_empty_list_of_texts() {
    let cases = [
        (
            "    - command: python3 script.py",
            "pipelines.p[0].command",
            "attendu une liste d'arguments (le programme puis ses arguments), trouvé un texte",
        ),
        (
            "    - command",
            "pipelines.p[0].command",
            "attendu une liste d'arguments (le programme puis ses arguments), trouvé rien",
        ),
        (
            "    - command: []",
            "pipelines.p[0].command",
            "liste vide : au moins le programme à lancer",
        ),
        (
            "    - command: [sleep, 5]",
            "pipelines.p[0].command[1]",
            "attendu un texte, trouvé un nombre (mettre entre guillemets)",
        ),
        (
            "    - command: [sh, -c, [x]]",
            "pipelines.p[0].command[2]",
            "attendu un texte, trouvé une liste",
        ),
    ];
    for (steps, field, reason) in cases {
        assert_eq!(
            config_error(steps),
            ConfigError {
                field: field.into(),
                reason: reason.into(),
            },
            "{steps}"
        );
    }
}

#[tokio::test]
async fn a_background_process_holding_the_output_fails_at_the_timeout_and_says_so() {
    let engine = engine_with("    - command: [sh, -c, 'echo ok; sleep 3 &']\n      timeout: 1");
    let Outcome::Failed(failure) = engine.on_copy("abc").await else {
        panic!("attendu un échec");
    };
    assert_eq!(
        failure.message,
        "délai dépassé (1 s) : le programme a fini, mais un processus lancé en \
         arrière-plan garde sa sortie ouverte"
    );
}

#[tokio::test]
async fn a_background_process_with_its_output_redirected_does_not_hold_the_step() {
    let engine = engine_with(
        "    - command: [sh, -c, 'echo ok; sleep 3 >/dev/null 2>&1 &']\n      timeout: 1",
    );
    assert_eq!(engine.on_copy("abc").await, write("ok"));
}

#[tokio::test]
async fn an_endless_output_fails_on_the_size_cap_before_the_timeout() {
    let engine = engine_with("    - command: ['yes']\n      timeout: 5");
    let started = Instant::now();
    let Outcome::Failed(failure) = engine.on_copy("abc").await else {
        panic!("attendu un échec");
    };
    assert_eq!(
        failure.message,
        "sortie standard trop grande (plus de 8 Mo)"
    );
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[tokio::test]
async fn a_long_error_output_is_cut_with_a_visible_marker() {
    let engine = engine_with("    - command: [sh, -c, 'yes erreur | head -c 200000 >&2; exit 1']");
    let Outcome::Failed(failure) = engine.on_copy("abc").await else {
        panic!("attendu un échec");
    };
    let marker = "\n[… sortie d'erreur coupée après 64 Ko]\n";
    let kept = failure
        .stderr
        .strip_suffix(marker)
        .expect("marqueur absent");
    assert_eq!(kept.len(), 64_000);
    assert!(kept.starts_with("erreur\nerreur\n"));
}

#[tokio::test]
async fn without_timeout_a_step_gets_ten_seconds() {
    let engine = engine_with("    - command: [sleep, '30']");
    let started = Instant::now();
    let Outcome::Failed(failure) = engine.on_copy("abc").await else {
        panic!("attendu un échec");
    };
    assert_eq!(failure.message, "délai dépassé (10 s)");
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_secs(10) && elapsed < Duration::from_secs(12));
}
