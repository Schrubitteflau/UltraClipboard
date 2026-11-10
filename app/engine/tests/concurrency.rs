//! Copies qui se chevauchent, et échec d'une étape.

use std::sync::Arc;
use std::time::Duration;

use engine::{Engine, Failure, Outcome, Pipeline, StepError, StepFuture, Transformation};
use tokio::sync::{Notify, Semaphore};

/// Met en majuscules ; sur « slow », prévient le test (`reached`) puis bloque
/// jusqu'à ce qu'il ouvre la barrière (`gate`).
struct GatedUpper {
    reached: Arc<Notify>,
    gate: Arc<Semaphore>,
}

impl Transformation for GatedUpper {
    fn name(&self) -> &str {
        "gated-upper"
    }

    fn apply<'a>(&'a self, input: &'a str) -> StepFuture<'a> {
        Box::pin(async move {
            if input == "slow" {
                self.reached.notify_one();
                let _permit = self.gate.acquire().await.unwrap();
            }
            Ok(input.to_uppercase())
        })
    }
}

struct Broken;

impl Transformation for Broken {
    fn name(&self) -> &str {
        "broken"
    }

    fn apply<'a>(&'a self, _input: &'a str) -> StepFuture<'a> {
        Box::pin(async {
            Err(StepError {
                message: "exit status 1".into(),
                stderr: "boom".into(),
            })
        })
    }
}

#[tokio::test]
async fn a_copy_during_a_slow_one_is_processed_without_waiting() {
    let reached = Arc::new(Notify::new());
    let gate = Arc::new(Semaphore::new(0));
    let engine = Engine::with_pipeline(Pipeline::new(vec![Box::new(GatedUpper {
        reached: reached.clone(),
        gate: gate.clone(),
    })]));

    let slow = tokio::spawn(engine.on_copy("slow"));
    // La copie lente est bien en cours de traitement avant la suivante.
    reached.notified().await;
    let fast = tokio::time::timeout(Duration::from_secs(2), engine.on_copy("abc"))
        .await
        .expect("la seconde copie a attendu la fin de la première");
    assert_eq!(fast, Outcome::Write("ABC".into()));

    gate.add_permits(1);
    assert_eq!(slow.await.unwrap(), Outcome::Cancelled);
}

#[tokio::test]
async fn a_failing_step_leaves_the_clipboard_alone_and_says_why() {
    let engine = Engine::with_pipeline(Pipeline::new(vec![Box::new(Broken)]));
    assert_eq!(
        engine.on_copy("abc").await,
        Outcome::Failed(Failure {
            step: "broken".into(),
            message: "exit status 1".into(),
            stderr: "boom".into(),
        })
    );
}
