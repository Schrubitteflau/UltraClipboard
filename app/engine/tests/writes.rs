//! Écritures publiées par le moteur ([`Engine::writes`]) : chaque texte à écrire
//! part au moment où l'écriture est décidée, dans l'ordre des décisions (US12).

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Wake, Waker};

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

/// Au réveil, relève ce que [`Engine::writes`] a déjà publié. Le réveil a lieu
/// dans l'appel qui notifie ou ajoute à l'historique, au milieu du traitement
/// de la copie : la relève dit ce qui était publié à cet instant précis.
struct Probe {
    writes: Mutex<UnboundedReceiver<String>>,
    seen: Mutex<Vec<Vec<String>>>,
}

impl Probe {
    fn new(engine: &Engine) -> Arc<Self> {
        Arc::new(Self {
            writes: Mutex::new(engine.writes()),
            seen: Mutex::default(),
        })
    }

    /// Fait attendre `future` (qui doit rester en attente) avec cette sonde
    /// comme réveil.
    fn arm<F: Future + Unpin>(self: &Arc<Self>, future: &mut F) {
        let waker = Waker::from(Arc::clone(self));
        let mut cx = Context::from_waker(&waker);
        assert!(std::pin::Pin::new(future).poll(&mut cx).is_pending());
    }

    fn seen(&self) -> Vec<Vec<String>> {
        self.seen.lock().unwrap().clone()
    }
}

impl Wake for Probe {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        let published = published(&mut self.writes.lock().unwrap());
        self.seen.lock().unwrap().push(published);
    }
}

// Le résultat est publié sous le verrou de l'état, au moment où l'écriture est
// décidée : donc déjà publié quand la notification de succès part ou quand
// l'entrée d'historique est ajoutée, qui suivent la décision dans la même
// tâche. Une publication faite juste après le déverrouillage, mais avant la
// notification, échapperait à ces tests : aucun observable ne la sépare de
// la décision sans dépendre de l'ordonnancement. Pour la restauration, il n'y a
// ni notification ni historique : seul « publiée quand `restore_original`
// rend la main » se vérifie (plus haut).

#[tokio::test]
async fn the_result_is_already_published_when_its_success_is_notified() {
    let yaml = "config_version: 1\nnotifications: all\npipelines:\n  shout: [upper]\nrules:\n  - match: ''\n    pipeline: shout\n";
    let engine = Engine::new(Config::parse(yaml).unwrap());
    let probe = Probe::new(&engine);
    let mut notices = engine.subscribe_notifications();
    let mut notified = Box::pin(notices.recv());
    probe.arm(&mut notified);
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("ABC".into()));
    assert_eq!(probe.seen(), [["ABC"]]);
}

#[tokio::test]
async fn the_result_is_already_published_when_it_enters_the_history() {
    let engine = shout();
    let probe = Probe::new(&engine);
    let mut changes = engine.history_changes();
    let mut recorded = Box::pin(changes.changed());
    probe.arm(&mut recorded);
    assert_eq!(engine.on_copy("abc").await, Outcome::Write("ABC".into()));
    assert_eq!(probe.seen(), [["ABC"]]);
}
