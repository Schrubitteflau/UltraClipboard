//! Comportement d'une copie, vu par l'interface publique du moteur.

use engine::{Engine, Outcome, Skip};

#[tokio::test]
async fn transforms_a_copy() {
    let engine = Engine::new();
    assert_eq!(
        engine.on_copy("hello").await,
        Outcome::Write("HELLO".into())
    );
}

#[tokio::test]
async fn ignores_its_own_output_once() {
    let engine = Engine::new();
    let Outcome::Write(out) = engine.on_copy("hello").await else {
        panic!("la copie aurait dû être transformée");
    };
    // L'écriture du résultat déclenche une nouvelle « copie » : on l'ignore.
    assert_eq!(engine.on_copy(&out).await, Outcome::Nothing(Skip::OwnWrite));
}

#[tokio::test]
async fn leaves_clipboard_alone_when_nothing_changes() {
    let engine = Engine::new();
    assert_eq!(
        engine.on_copy("ALREADY").await,
        Outcome::Nothing(Skip::Unchanged)
    );
}

#[tokio::test]
async fn a_new_copy_after_our_output_is_transformed() {
    let engine = Engine::new();
    engine.on_copy("a").await;
    engine.on_copy("A").await;
    assert_eq!(engine.on_copy("b").await, Outcome::Write("B".into()));
}

#[tokio::test]
async fn our_write_is_ignored_only_once() {
    let engine = Engine::new();
    assert_eq!(engine.on_copy("a").await, Outcome::Write("A".into()));
    assert_eq!(engine.on_copy("A").await, Outcome::Nothing(Skip::OwnWrite));
    // Une nouvelle copie de « A » par l'utilisateur est traitée normalement.
    assert_eq!(engine.on_copy("A").await, Outcome::Nothing(Skip::Unchanged));
}
