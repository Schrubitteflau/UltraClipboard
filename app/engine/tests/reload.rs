//! Remplacer la config active : les copies suivantes suivent la nouvelle config,
//! une config refusée laisse l'ancienne active, l'état du moteur est conservé.

use std::sync::Arc;

use engine::{Config, Engine, Outcome, Pipeline, Skip, StepFuture, Transformation};
use tokio::sync::Semaphore;

const UPPER: &str =
    "config_version: 1\npipelines:\n  p: [upper]\nrules:\n  - match: ''\n    pipeline: p\n";
const LOWER: &str =
    "config_version: 1\npipelines:\n  p: [lower]\nrules:\n  - match: ''\n    pipeline: p\n";

fn engine(yaml: &str) -> Engine {
    Engine::new(Config::parse(yaml).unwrap())
}

#[tokio::test]
async fn a_new_valid_config_drives_the_next_copies() {
    let engine = engine(UPPER);
    assert_eq!(engine.on_copy("Ab").await, Outcome::Write("AB".into()));

    engine.load(LOWER).unwrap();
    assert_eq!(engine.on_copy("Cd").await, Outcome::Write("cd".into()));
    assert_eq!(engine.config_error(), None);
}

#[tokio::test]
async fn an_invalid_config_is_refused_and_the_active_one_stays() {
    let engine = engine(UPPER);
    let broken =
        "config_version: 1\npipelines:\n  p: [lower]\nrules:\n  - match: '('\n    pipeline: p\n";

    let err = engine.load(broken).unwrap_err();
    assert_eq!(err.field, "rules[0].match");
    assert_eq!(engine.config_error(), Some(err));
    assert_eq!(engine.on_copy("Ab").await, Outcome::Write("AB".into()));
}

#[tokio::test]
async fn a_valid_config_after_a_refused_one_clears_the_error() {
    let engine = engine(UPPER);
    engine.load("config_version: 2\n").unwrap_err();

    engine.load(LOWER).unwrap();
    assert_eq!(engine.config_error(), None);
    assert_eq!(engine.on_copy("Ab").await, Outcome::Write("ab".into()));
}

#[tokio::test]
async fn an_invalid_first_config_leaves_no_rule_and_reports_the_error() {
    let engine = Engine::new(Config::default());
    engine.load("rules: []\n").unwrap_err();
    assert_eq!(engine.config_error().unwrap().field, "config_version");
    assert_eq!(engine.on_copy("Ab").await, Outcome::Nothing(Skip::NoRule));
}

/// L'écho de ce qu'UltraClipboard a écrit avant le rechargement reste reconnu.
#[tokio::test]
async fn reloading_keeps_the_echo_of_our_last_write() {
    let engine = engine(UPPER);
    assert_eq!(engine.on_copy("ab").await, Outcome::Write("AB".into()));

    engine.load(LOWER).unwrap();
    assert_eq!(engine.on_copy("AB").await, Outcome::Nothing(Skip::OwnWrite));
}

/// Met en majuscules une fois que le test ouvre la barrière.
struct GatedUpper(Arc<Semaphore>);

impl Transformation for GatedUpper {
    fn name(&self) -> &str {
        "gated-upper"
    }

    fn apply<'a>(&'a self, input: &'a str) -> StepFuture<'a> {
        Box::pin(async move {
            let _permit = self.0.acquire().await.unwrap();
            Ok(input.to_uppercase())
        })
    }
}

/// Une copie en cours finit avec le Pipeline qu'elle avait au moment de la copie.
#[tokio::test]
async fn a_copy_in_progress_finishes_with_its_pipeline() {
    let gate = Arc::new(Semaphore::new(0));
    let engine = Engine::with_pipeline(Pipeline::new(vec![Box::new(GatedUpper(gate.clone()))]));
    let slow = tokio::spawn(engine.on_copy("ab"));
    tokio::task::yield_now().await;

    engine.load(LOWER).unwrap();
    gate.add_permits(1);
    assert_eq!(slow.await.unwrap(), Outcome::Write("AB".into()));
    assert_eq!(engine.on_copy("Cd").await, Outcome::Write("cd".into()));
}
