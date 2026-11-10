//! Écho de notre propre écriture qui revient après une nouvelle copie.

mod common;

use common::{pids_written_to, scratch};
use engine::{Config, Engine, Outcome, Skip};

/// a est écrit ; l'utilisateur copie « slow » avant que l'écho de « A » revienne.
#[tokio::test]
async fn echo_arriving_after_a_new_copy_does_not_cancel_it() {
    let pidfile = scratch("echo.pid");
    let go = scratch("echo.go");
    // « slow » attend que le test crée `go` avant d'être mis en majuscules ; le
    // reste l'est aussitôt.
    let yaml = format!(
        "config_version: 1\npipelines:\n  slow:\n    - command: [sh, -c, 'echo $$ > \"$0\"; while [ ! -e \"$1\" ]; do sleep 0.02; done; cat', '{}', '{}']\n      timeout: 5\n    - upper\n  shout: [upper]\nrules:\n  - match: '^slow$'\n    pipeline: slow\n  - match: ''\n    pipeline: shout\n",
        pidfile.display(),
        go.display()
    );
    let engine = Engine::new(Config::parse(&yaml).unwrap());
    assert_eq!(engine.on_copy("a").await, Outcome::Write("A".into()));
    let slow = tokio::spawn(engine.on_copy("slow"));
    // La commande de « slow » tourne quand l'écho arrive.
    pids_written_to(&pidfile, 1).await;
    let echo = engine.on_copy("A").await;
    assert_eq!(echo, Outcome::Nothing(Skip::OwnWrite));
    std::fs::write(&go, "").unwrap();
    assert_eq!(slow.await.unwrap(), Outcome::Write("SLOW".into()));
    std::fs::remove_file(&pidfile).unwrap();
    std::fs::remove_file(&go).unwrap();
}
