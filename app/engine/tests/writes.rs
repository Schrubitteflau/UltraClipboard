//! Écritures publiées par le moteur ([`Engine::writes`]) : chaque texte à écrire
//! part au moment où l'écriture est décidée, dans l'ordre des décisions (US12).

use engine::{Config, Engine, Outcome};
use tokio::sync::mpsc::UnboundedReceiver;

/// Unique Règle : toute copie est mise en majuscules.
fn shout() -> Engine {
    let yaml = "config_version: 1\npipelines:\n  shout: [upper]\nrules:\n  - match: ''\n    pipeline: shout\n";
    Engine::new(Config::parse(yaml).unwrap())
}

/// Tout ce qui a été publié jusqu'ici, sans attendre.
fn published(writes: &mut UnboundedReceiver<String>) -> Vec<String> {
    std::iter::from_fn(|| writes.try_recv().ok()).collect()
}

#[tokio::test]
async fn results_are_published_in_the_order_they_are_decided() {
    let engine = shout();
    let mut writes = engine.writes();
    assert_eq!(engine.on_copy("c3").await, Outcome::Write("C3".into()));
    // Publié dès la décision, avant que l'appelant fasse quoi que ce soit.
    assert_eq!(published(&mut writes), ["C3"]);
    assert_eq!(engine.on_copy("c4").await, Outcome::Write("C4".into()));
    assert_eq!(published(&mut writes), ["C4"]);
}

#[tokio::test]
async fn a_superseded_copy_publishes_nothing_even_if_it_finishes_last() {
    let engine = shout();
    let mut writes = engine.writes();
    let older = engine.on_copy("c3");
    let newer = engine.on_copy("c4");
    assert_eq!(newer.await, Outcome::Write("C4".into()));
    assert_eq!(older.await, Outcome::Cancelled);
    assert_eq!(published(&mut writes), ["C4"]);
}

#[tokio::test]
async fn a_restore_decided_after_a_result_is_published_after_it() {
    let engine = shout();
    let mut writes = engine.writes();
    engine.on_copy("abc").await;
    assert_eq!(engine.restore_original().as_deref(), Some("abc"));
    assert_eq!(published(&mut writes), ["ABC", "abc"]);
}

#[tokio::test]
async fn nothing_is_published_when_nothing_is_written() {
    let engine = shout();
    let mut writes = engine.writes();
    assert_eq!(engine.restore_original(), None);
    engine.on_copy("abc").await;
    // L'écho de notre écriture : rien à écrire.
    engine.on_copy("ABC").await;
    assert_eq!(published(&mut writes), ["ABC"]);
}

#[tokio::test]
async fn only_writes_decided_after_subscribing_are_received() {
    let engine = shout();
    engine.on_copy("abc").await;
    let mut writes = engine.writes();
    engine.on_copy("def").await;
    assert_eq!(published(&mut writes), ["DEF"]);
}
