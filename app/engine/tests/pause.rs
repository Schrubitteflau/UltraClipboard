//! Pause et réapplication : en pause aucune copie n'est transformée ; réappliquer
//! est une action explicite qui passe outre la pause et l'anti-boucle.

mod common;

use common::{pids_written_to, scratch, wait_dead};
use engine::{Config, Engine, HistoryStatus, Outcome, Skip};

/// Unique Règle : toute copie est mise en majuscules.
fn shout() -> Engine {
    let yaml = "config_version: 1\npipelines:\n  shout: [upper]\nrules:\n  - match: ''\n    pipeline: shout\n";
    Engine::new(Config::parse(yaml).unwrap())
}

#[tokio::test]
async fn no_copy_is_transformed_while_paused() {
    let engine = shout();
    engine.set_paused(true);
    assert!(engine.paused());
    assert_eq!(engine.on_copy("abc").await, Outcome::Nothing(Skip::Paused));
}

#[tokio::test]
async fn copies_are_transformed_again_after_resuming() {
    let engine = shout();
    engine.set_paused(true);
    engine.set_paused(false);
    assert!(!engine.paused());
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("ABC".into()));
}

#[tokio::test]
async fn a_copy_made_while_paused_is_not_in_the_history() {
    let engine = shout();
    engine.set_paused(true);
    engine.on_copy("abc").await;
    assert_eq!(engine.history(), vec![]);
}

#[tokio::test]
async fn the_original_can_be_restored_while_paused() {
    let engine = shout();
    engine.on_copy("abc").await;
    engine.on_copy("ABC").await;
    engine.set_paused(true);
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
    // Son écho est ignoré comme hors pause.
    assert_eq!(
        engine.on_copy("abc").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
}

// D39 : en pause, une copie reste la dernière copie ; seule la transformation
// est suspendue.

#[tokio::test]
async fn restore_never_overwrites_a_copy_made_while_paused() {
    let engine = shout();
    engine.on_copy("abc").await;
    engine.on_copy("ABC").await;
    engine.set_paused(true);
    engine.on_copy("def").await;
    assert_eq!(engine.restore_original(), None);
}

#[tokio::test]
async fn a_copy_made_while_paused_stops_the_copy_in_progress() {
    let pidfile = scratch("pause-in-progress.pid");
    let yaml = format!(
        "config_version: 1\npipelines:\n  slow:\n    - command: [sh, -c, 'echo $$ > \"$0\"; exec sleep 30', '{}']\n      timeout: 5\nrules:\n  - match: '^slow$'\n    pipeline: slow\n",
        pidfile.display()
    );
    let engine = Engine::new(Config::parse(&yaml).unwrap());
    let slow = tokio::spawn(engine.on_copy("slow"));
    let pids = pids_written_to(&pidfile, 1).await;

    engine.set_paused(true);
    assert_eq!(engine.on_copy("def").await, Outcome::Nothing(Skip::Paused));
    assert_eq!(slow.await.unwrap(), Outcome::Cancelled);
    assert!(
        wait_dead(pids[0]).await,
        "la commande de la copie arrêtée vit encore"
    );
    std::fs::remove_file(&pidfile).unwrap();
}

#[tokio::test]
async fn pausing_and_resuming_are_signalled() {
    let engine = shout();
    let mut changes = engine.pause_changes();
    engine.set_paused(true);
    assert!(changes.has_changed().unwrap());
    assert!(*changes.borrow_and_update());
    // Déjà en pause : rien de neuf.
    engine.set_paused(true);
    assert!(!changes.has_changed().unwrap());
    engine.set_paused(false);
    assert!(!*changes.borrow_and_update());
}

// Réappliquer (D33) : même issue qu'une copie, mais action explicite.

#[tokio::test]
async fn reapply_transforms_while_paused() {
    let engine = shout();
    engine.set_paused(true);
    assert_eq!(engine.reapply("abc").await, Outcome::Write("ABC".into()));
}

#[tokio::test]
async fn reapply_transforms_our_own_write() {
    // Une étape non idempotente : « abc! » redevient « abc!! ».
    let yaml = "config_version: 1\npipelines:\n  exclaim:\n    - command: [sh, -c, 'cat; printf !']\nrules:\n  - match: ''\n    pipeline: exclaim\n";
    let engine = Engine::new(Config::parse(yaml).unwrap());
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("abc!".into()));
    // Le texte écrit est réappliqué avant que son écho revienne.
    assert_eq!(engine.reapply("abc!").await, Outcome::Write("abc!!".into()));
    // L'écho de ce nouveau résultat est ignoré comme celui d'une copie.
    assert_eq!(
        engine.on_copy("abc!!").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
}

#[tokio::test]
async fn reapply_without_matching_rule_does_nothing() {
    let engine = Engine::new(Config::default());
    assert_eq!(engine.reapply("abc").await, Outcome::Nothing(Skip::NoRule));
}

#[tokio::test]
async fn the_original_of_a_reapply_can_be_restored() {
    let engine = shout();
    engine.set_paused(true);
    engine.on_copy("abc").await;
    engine.reapply("abc").await;
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
}

#[tokio::test]
async fn a_reapply_enters_the_history_like_a_copy() {
    let engine = shout();
    engine.set_paused(true);
    engine.reapply("abc").await;
    let history = engine.history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].original, "abc");
    assert_eq!(
        history[0].status,
        HistoryStatus::Success {
            result: "ABC".into()
        }
    );
}

#[tokio::test]
async fn reapply_stops_the_copy_in_progress() {
    let pidfile = scratch("reapply-in-progress.pid");
    let yaml = format!(
        "config_version: 1\npipelines:\n  slow:\n    - command: [sh, -c, 'echo $$ > \"$0\"; exec sleep 30', '{}']\n      timeout: 5\n  shout: [upper]\nrules:\n  - match: '^slow$'\n    pipeline: slow\n  - match: ''\n    pipeline: shout\n",
        pidfile.display()
    );
    let engine = Engine::new(Config::parse(&yaml).unwrap());
    let slow = tokio::spawn(engine.on_copy("slow"));
    let pids = pids_written_to(&pidfile, 1).await;

    assert_eq!(engine.reapply("abc").await, Outcome::Write("ABC".into()));
    assert_eq!(slow.await.unwrap(), Outcome::Cancelled);
    assert!(
        wait_dead(pids[0]).await,
        "la commande de la copie arrêtée vit encore"
    );
    std::fs::remove_file(&pidfile).unwrap();
}

#[tokio::test]
async fn pausing_cancels_the_copy_in_progress() {
    let pidfile = scratch("pause-cancels.pid");
    let yaml = format!(
        "config_version: 1\nnotifications: all\npipelines:\n  slow:\n    - command: [sh, -c, 'echo $$ > \"$0\"; sleep 1; echo DONE', '{}']\n      timeout: 5\nrules:\n  - match: '^slow$'\n    pipeline: slow\n",
        pidfile.display()
    );
    let engine = Engine::new(Config::parse(&yaml).unwrap());
    let mut notices = engine.subscribe_notifications();
    let slow = tokio::spawn(engine.on_copy("slow"));
    let pids = pids_written_to(&pidfile, 1).await;

    engine.set_paused(true);
    assert_eq!(slow.await.unwrap(), Outcome::Cancelled);
    assert!(
        wait_dead(pids[0]).await,
        "la commande de la copie arrêtée vit encore"
    );
    assert_eq!(engine.history()[0].status, HistoryStatus::Cancelled);
    assert!(notices.try_recv().is_err(), "une notification est partie");
    std::fs::remove_file(&pidfile).unwrap();
}

/// Unique Règle : toute copie reçoit un « ! » de plus (étape non idempotente,
/// qui révèle tout écho retraité).
fn exclaim() -> Engine {
    let yaml = "config_version: 1\npipelines:\n  exclaim:\n    - command: [sh, -c, 'cat; printf !']\nrules:\n  - match: ''\n    pipeline: exclaim\n";
    Engine::new(Config::parse(yaml).unwrap())
}

#[tokio::test]
async fn a_reapply_keeps_the_echo_of_an_earlier_write_awaited() {
    let engine = exclaim();
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("abc!".into()));
    assert_eq!(engine.reapply("abc!").await, Outcome::Write("abc!!".into()));
    // L'écho de « abc! » arrive après la réapplication.
    assert_eq!(
        engine.on_copy("abc!").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
    assert_eq!(
        engine.on_copy("abc!!").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
}

#[tokio::test]
async fn a_reapply_in_pause_keeps_the_echo_of_an_earlier_write_awaited() {
    let engine = exclaim();
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("abc!".into()));
    engine.set_paused(true);
    assert_eq!(engine.reapply("abc!").await, Outcome::Write("abc!!".into()));
    assert_eq!(
        engine.on_copy("abc!").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
    assert_eq!(engine.restore_original().as_deref(), Some("abc!"));
}

#[tokio::test]
async fn a_reapply_after_a_restore_keeps_both_echoes_awaited() {
    let engine = exclaim();
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("abc!".into()));
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
    assert_eq!(engine.reapply("abc").await, Outcome::Write("abc!".into()));
    // Les trois échos reviennent en retard : aucun n'est retraité.
    for echo in ["abc!", "abc", "abc!"] {
        assert_eq!(
            engine.on_copy(echo).await,
            Outcome::Nothing(Skip::OwnWrite),
            "{echo}"
        );
    }
}

#[tokio::test]
async fn restore_after_reapply_in_pause_with_echo() {
    let engine = exclaim();
    engine.set_paused(true);
    assert_eq!(engine.on_copy("abc").await, Outcome::Nothing(Skip::Paused));
    assert_eq!(engine.reapply("abc").await, Outcome::Write("abc!".into()));
    assert_eq!(
        engine.on_copy("abc!").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
    assert_eq!(
        engine.on_copy("abc").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
}

#[tokio::test]
async fn a_reapply_that_transforms_nothing_keeps_the_original() {
    // Seul « abc » a une Règle : « ABC » réappliqué ne donne rien.
    let yaml = "config_version: 1\npipelines:\n  shout: [upper]\nrules:\n  - match: '^[a-c]+$'\n    pipeline: shout\n";
    let engine = Engine::new(Config::parse(yaml).unwrap());
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("ABC".into()));
    engine.on_copy("ABC").await;
    assert_eq!(engine.reapply("ABC").await, Outcome::Nothing(Skip::NoRule));
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
}

/// « slow » écrit son numéro de processus dans `pidfile` puis dort ; le reste
/// est mis en majuscules.
fn slow_or_shout(pidfile: &std::path::Path) -> Engine {
    let yaml = format!(
        "config_version: 1\nnotifications: all\npipelines:\n  slow:\n    - command: [sh, -c, 'echo $$ > \"$0\"; exec sleep 30', '{}']\n      timeout: 5\n  shout: [upper]\nrules:\n  - match: '^slow$'\n    pipeline: slow\n  - match: ''\n    pipeline: shout\n",
        pidfile.display()
    );
    Engine::new(Config::parse(&yaml).unwrap())
}

#[tokio::test]
async fn restore_stops_a_reapply_in_progress_and_writes_the_original() {
    let pidfile = scratch("restore-stops-reapply.pid");
    let engine = slow_or_shout(&pidfile);
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("ABC".into()));
    let reapply = tokio::spawn(engine.reapply("slow"));
    let pids = pids_written_to(&pidfile, 1).await;

    // Une réapplication n'est pas une copie : l'Original d'avant elle revient.
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
    assert_eq!(reapply.await.unwrap(), Outcome::Cancelled);
    assert!(
        wait_dead(pids[0]).await,
        "la commande de la réapplication arrêtée vit encore"
    );
    // Les échos du résultat et de la restauration arrivent en retard.
    for echo in ["ABC", "abc"] {
        assert_eq!(
            engine.on_copy(echo).await,
            Outcome::Nothing(Skip::OwnWrite),
            "{echo}"
        );
    }
    std::fs::remove_file(&pidfile).unwrap();
}

#[tokio::test]
async fn pausing_cancels_a_reapply_in_progress() {
    let pidfile = scratch("pause-cancels-reapply.pid");
    let engine = slow_or_shout(&pidfile);
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("ABC".into()));
    engine.on_copy("ABC").await;
    let mut notices = engine.subscribe_notifications();
    let reapply = tokio::spawn(engine.reapply("slow"));
    let pids = pids_written_to(&pidfile, 1).await;

    engine.set_paused(true);
    assert_eq!(reapply.await.unwrap(), Outcome::Cancelled);
    assert!(
        wait_dead(pids[0]).await,
        "la commande de la réapplication arrêtée vit encore"
    );
    assert_eq!(engine.history()[0].status, HistoryStatus::Cancelled);
    assert!(notices.try_recv().is_err(), "une notification est partie");
    // La réapplication n'était pas une copie : l'Original d'avant reste.
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
    std::fs::remove_file(&pidfile).unwrap();
}
