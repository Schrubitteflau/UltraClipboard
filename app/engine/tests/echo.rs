//! Écho de notre propre écriture qui revient après une nouvelle copie.

use engine::{Engine, Outcome, Pipeline, Skip, StepFuture, Transformation};
use std::sync::Arc;
use tokio::sync::Semaphore;
struct Gated(Arc<Semaphore>);
impl Transformation for Gated {
    fn name(&self) -> &str {
        "gated"
    }
    fn apply<'a>(&'a self, input: &'a str) -> StepFuture<'a> {
        Box::pin(async move {
            if input == "slow" {
                let _p = self.0.acquire().await.unwrap();
            }
            Ok(input.to_uppercase())
        })
    }
}
/// a est écrit ; l'utilisateur copie « slow » avant que l'écho de « A » revienne.
#[tokio::test]
async fn echo_arriving_after_a_new_copy_does_not_cancel_it() {
    let gate = Arc::new(Semaphore::new(0));
    let engine = Engine::with_pipeline(Pipeline::new(vec![Box::new(Gated(gate.clone()))]));
    assert_eq!(engine.on_copy("a").await, Outcome::Write("A".into()));
    let slow = tokio::spawn(engine.on_copy("slow"));
    let echo = engine.on_copy("A").await;
    gate.add_permits(1);
    let slow = slow.await.unwrap();
    assert_eq!(echo, Outcome::Nothing(Skip::OwnWrite));
    assert_eq!(slow, Outcome::Write("SLOW".into()));
}
