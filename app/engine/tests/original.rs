//! Original et restauration : annuler la dernière Transformation appliquée.

mod common;

use common::{pids_written_to, scratch, wait_dead};
use engine::{Config, Engine, Outcome, Skip};

/// Unique Règle : toute copie est mise en majuscules.
fn shout() -> Engine {
    let yaml = "config_version: 1\npipelines:\n  shout: [upper]\nrules:\n  - match: ''\n    pipeline: shout\n";
    Engine::new(Config::parse(yaml).unwrap())
}

#[tokio::test]
async fn restore_gives_back_the_text_before_the_transformation() {
    let engine = shout();
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("ABC".into()));
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
}

#[tokio::test]
async fn restore_without_any_transformation_does_nothing() {
    let engine = shout();
    assert_eq!(engine.restore_original(), None);
}

#[tokio::test]
async fn a_copy_left_as_is_is_not_an_original() {
    // Aucune Règle : la copie reste telle quelle, rien n'est appliqué.
    let engine = Engine::new(Config::default());
    assert_eq!(engine.on_copy("abc").await, Outcome::Nothing(Skip::NoRule));
    assert_eq!(engine.restore_original(), None);
}

#[tokio::test]
async fn the_restored_original_is_not_transformed_again() {
    let engine = shout();
    engine.on_copy("abc").await;
    // L'écho du résultat, puis la restauration et son écho.
    assert_eq!(
        engine.on_copy("ABC").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
    engine.restore_original();
    assert_eq!(
        engine.on_copy("abc").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
}

#[tokio::test]
async fn restoring_twice_writes_the_original_again() {
    let engine = shout();
    engine.on_copy("abc").await;
    engine.on_copy("ABC").await;
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
    engine.on_copy("abc").await;
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
    // Son écho est ignoré lui aussi.
    assert_eq!(
        engine.on_copy("abc").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
}

#[tokio::test]
async fn the_original_survives_a_config_reload() {
    let engine = shout();
    engine.on_copy("abc").await;
    engine
        .load("config_version: 1\npipelines:\n  quiet: [lower]\nrules:\n  - match: ''\n    pipeline: quiet\n")
        .unwrap();
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
}

#[tokio::test]
async fn the_original_follows_the_last_transformation() {
    let engine = shout();
    engine.on_copy("abc").await;
    engine.on_copy("ABC").await;
    assert_eq!(engine.on_copy("def").await, Outcome::Write("DEF".into()));
    assert_eq!(engine.restore_original().as_deref(), Some("def"));
}

// D39 : la dernière copie gagne aussi face à la restauration.

#[tokio::test]
async fn restore_never_overwrites_a_newer_copy_left_as_is() {
    // « keep » n'a pas de Règle : il reste tel que copié.
    let yaml = "config_version: 1\npipelines:\n  shout: [upper]\nrules:\n  - match: '^[a-c]+$'\n    pipeline: shout\n";
    let engine = Engine::new(Config::parse(yaml).unwrap());
    engine.on_copy("abc").await;
    engine.on_copy("ABC").await;
    assert_eq!(engine.on_copy("keep").await, Outcome::Nothing(Skip::NoRule));
    assert_eq!(engine.restore_original(), None);
}

#[tokio::test]
async fn restore_never_overwrites_a_newer_copy_without_text() {
    let engine = shout();
    engine.on_copy("abc").await;
    engine.on_copy("ABC").await;
    engine.on_copy_without_text();
    assert_eq!(engine.restore_original(), None);
}

#[tokio::test]
async fn restore_stops_a_newer_copy_in_progress_and_leaves_it_as_copied() {
    let pidfile = scratch("restore-in-progress.pid");
    let yaml = format!(
        "config_version: 1\npipelines:\n  slow:\n    - command: [sh, -c, 'echo $$ > \"$0\"; exec sleep 30', '{}']\n      timeout: 5\n  shout: [upper]\nrules:\n  - match: '^slow$'\n    pipeline: slow\n  - match: ''\n    pipeline: shout\n",
        pidfile.display()
    );
    let engine = Engine::new(Config::parse(&yaml).unwrap());
    engine.on_copy("abc").await;
    engine.on_copy("ABC").await;
    let slow = tokio::spawn(engine.on_copy("slow"));
    let pids = pids_written_to(&pidfile, 1).await;

    // « slow » reste tel que copié : rien à écrire, et surtout pas « abc ».
    assert_eq!(engine.restore_original(), None);
    assert_eq!(slow.await.unwrap(), Outcome::Cancelled);
    assert!(
        wait_dead(pids[0]).await,
        "la commande de la copie arrêtée vit encore"
    );
    // L'Original d'avant « slow » ne revient pas non plus après coup.
    assert_eq!(engine.restore_original(), None);
    std::fs::remove_file(&pidfile).unwrap();
}

#[tokio::test]
async fn restore_without_original_stops_a_copy_in_progress() {
    let pidfile = scratch("restore-no-original.pid");
    let yaml = format!(
        "config_version: 1\npipelines:\n  slow:\n    - command: [sh, -c, 'echo $$ > \"$0\"; exec sleep 30', '{}']\n      timeout: 5\nrules:\n  - match: '^slow$'\n    pipeline: slow\n",
        pidfile.display()
    );
    let engine = Engine::new(Config::parse(&yaml).unwrap());
    let slow = tokio::spawn(engine.on_copy("slow"));
    let pids = pids_written_to(&pidfile, 1).await;

    // Aucun Original : rien n'est écrit, mais « slow » reste tel que copié.
    assert_eq!(engine.restore_original(), None);
    assert_eq!(slow.await.unwrap(), Outcome::Cancelled);
    assert!(
        wait_dead(pids[0]).await,
        "la commande de la copie arrêtée vit encore"
    );
    std::fs::remove_file(&pidfile).unwrap();
}

// Échos en retard : une étape non idempotente (« abc » → « abc! » → « abc!! »)
// révèle tout écho de notre écriture retraité comme une copie.

/// Unique Règle : toute copie reçoit un « ! » de plus.
fn exclaim() -> Engine {
    let yaml = "config_version: 1\npipelines:\n  exclaim:\n    - command: [sh, -c, 'cat; printf !']\nrules:\n  - match: ''\n    pipeline: exclaim\n";
    Engine::new(Config::parse(yaml).unwrap())
}

#[tokio::test]
async fn the_echo_of_the_result_coming_back_after_a_restore_is_ignored() {
    let engine = exclaim();
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("abc!".into()));
    // Restauration avant le retour de l'écho de « abc! ».
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
    assert_eq!(
        engine.on_copy("abc!").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
    assert_eq!(
        engine.on_copy("abc").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
}

#[tokio::test]
async fn two_quick_restores_have_both_echoes_ignored() {
    let engine = exclaim();
    engine.on_copy("abc").await;
    engine.on_copy("abc!").await;
    // Deux restaurations avant le retour du premier écho.
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
    assert_eq!(
        engine.on_copy("abc").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
    assert_eq!(
        engine.on_copy("abc").await,
        Outcome::Nothing(Skip::OwnWrite)
    );
    // Chaque écho n'est ignoré qu'une fois : une vraie copie est traitée.
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("abc!".into()));
}

#[tokio::test]
async fn a_lost_echo_does_not_hide_a_later_real_copy_of_its_text() {
    let engine = exclaim();
    // L'écho de « abc! » ne revient jamais : l'utilisateur a copié « x » avant.
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("abc!".into()));
    assert_eq!(engine.on_copy("x").await, Outcome::Write("x!".into()));
    assert_eq!(engine.on_copy("x!").await, Outcome::Nothing(Skip::OwnWrite));
    // Plus tard, une vraie copie de « abc! » est traitée.
    assert_eq!(engine.on_copy("abc!").await, Outcome::Write("abc!!".into()));
}

#[tokio::test]
async fn at_most_four_echoes_are_awaited() {
    let engine = exclaim();
    // Cinq écritures sans écho : le résultat puis quatre restaurations.
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("abc!".into()));
    for _ in 0..4 {
        assert_eq!(engine.restore_original().as_deref(), Some("abc"));
    }
    // La plus ancienne est oubliée : son texte est traité comme une copie.
    assert_eq!(engine.on_copy("abc!").await, Outcome::Write("abc!!".into()));
}
