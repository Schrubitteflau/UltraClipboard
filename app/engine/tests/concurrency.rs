//! La dernière copie gagne : une copie plus récente arrête le Pipeline en cours,
//! avec de vrais processus.

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{pids_written_to, scratch, wait_dead};
use engine::{Config, Engine, Outcome};

/// Bien en deçà du délai des commandes lentes (5 s).
const PROMPTLY: Duration = Duration::from_secs(2);

/// « slow » passe par le Pipeline `steps` (étapes YAML en style bloc, indentées
/// de quatre espaces), dont le premier argument `"$0"` des commandes est
/// `pidfile` ; toute autre copie est mise en majuscules.
fn engine(steps: &str, pidfile: &Path) -> Engine {
    let steps = steps.replace("PIDFILE", &pidfile.display().to_string());
    let yaml = format!(
        "config_version: 1\npipelines:\n  slow:\n{steps}\n  shout: [upper]\nrules:\n  - match: '^slow$'\n    pipeline: slow\n  - match: ''\n    pipeline: shout\n"
    );
    Engine::new(Config::parse(&yaml).unwrap())
}

/// Commande qui écrit son numéro de processus puis dort 30 s (délai 5 s).
const SLEEPS: &str =
    "    - command: [sh, -c, 'echo $$ > \"$0\"; exec sleep 30', 'PIDFILE']\n      timeout: 5";

#[tokio::test]
async fn a_newer_copy_kills_the_running_command() {
    let pidfile = scratch("superseded.pid");
    let engine = engine(SLEEPS, &pidfile);
    let started = Instant::now();
    let slow = tokio::spawn(engine.on_copy("slow"));
    let pids = pids_written_to(&pidfile, 1).await;

    // La copie récente est traitée sans attendre la lente.
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("ABC".into()));
    assert_eq!(slow.await.unwrap(), Outcome::Cancelled);
    assert!(started.elapsed() < PROMPTLY, "{:?}", started.elapsed());
    assert!(
        wait_dead(pids[0]).await,
        "la commande de la copie dépassée vit encore"
    );
    std::fs::remove_file(&pidfile).unwrap();
}

#[tokio::test]
async fn a_newer_copy_kills_the_processes_the_command_started() {
    let pidfile = scratch("superseded-children.pid");
    let engine = engine(
        "    - command: [sh, -c, 'echo $$ > \"$0\"; sleep 30 & echo $! >> \"$0\"; wait', 'PIDFILE']\n      timeout: 5",
        &pidfile,
    );
    let started = Instant::now();
    let slow = tokio::spawn(engine.on_copy("slow"));
    let pids = pids_written_to(&pidfile, 2).await;

    engine.on_copy("abc").await;
    assert_eq!(slow.await.unwrap(), Outcome::Cancelled);
    // Avant le délai (5 s), qui tuerait aussi le groupe : c'est l'annulation.
    assert!(started.elapsed() < PROMPTLY, "{:?}", started.elapsed());
    for pid in pids {
        assert!(wait_dead(pid).await, "le processus {pid} vit encore");
    }
    std::fs::remove_file(&pidfile).unwrap();
}

#[tokio::test]
async fn a_cancelled_pipeline_never_starts_its_next_step() {
    let pidfile = scratch("next-step.pid");
    let marker = scratch("next-step-ran");
    // La première étape finirait seule après une demi-seconde.
    let engine = engine(
        &format!(
            "    - command: [sh, -c, 'echo $$ > \"$0\"; sleep 0.5; cat', 'PIDFILE']\n    - command: [touch, '{}']",
            marker.display()
        ),
        &pidfile,
    );
    let slow = tokio::spawn(engine.on_copy("slow"));
    pids_written_to(&pidfile, 1).await;

    engine.on_copy("abc").await;
    assert_eq!(slow.await.unwrap(), Outcome::Cancelled);
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(!marker.exists(), "l'étape suivante a tourné");
    std::fs::remove_file(&pidfile).unwrap();
}

#[tokio::test]
async fn a_copy_without_text_cancels_the_copy_in_progress() {
    let pidfile = scratch("non-text.pid");
    let engine = engine(SLEEPS, &pidfile);
    let started = Instant::now();
    let slow = tokio::spawn(engine.on_copy("slow"));
    let pids = pids_written_to(&pidfile, 1).await;

    // Une image copiée juste après : rien à écrire, mais la copie en cours s'arrête.
    engine.on_copy_without_text();
    assert_eq!(slow.await.unwrap(), Outcome::Cancelled);
    assert!(started.elapsed() < PROMPTLY, "{:?}", started.elapsed());
    assert!(
        wait_dead(pids[0]).await,
        "la commande de la copie annulée vit encore"
    );
    std::fs::remove_file(&pidfile).unwrap();
}
