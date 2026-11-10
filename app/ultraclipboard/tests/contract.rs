//! Contrat D-Bus entre l'extension et l'appli, sur un bus privé.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{Extension, PrivateBus, upper_everything};
use engine::{Config, Engine, Pipeline, StepFuture, Transformation};
use tokio::sync::Semaphore;
use ultraclipboard::{ServeError, serve};

const SOON: Duration = Duration::from_secs(5);

#[tokio::test]
async fn copied_produces_apply() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, upper_everything()).await.unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("ABC"));
}

#[tokio::test]
async fn a_second_instance_is_refused() {
    let bus = PrivateBus::start();
    let first = bus.connect().await;
    serve(&first, Engine::new(Config::default())).await.unwrap();

    let second = bus.connect().await;
    let err = serve(&second, Engine::new(Config::default()))
        .await
        .unwrap_err();
    assert!(matches!(err, ServeError::AlreadyRunning), "{err:?}");
}

/// Met en majuscules ; bloque sur « slow » jusqu'à ce que le test ouvre la barrière.
struct GatedUpper(Arc<Semaphore>);

impl Transformation for GatedUpper {
    fn name(&self) -> &str {
        "gated-upper"
    }

    fn apply<'a>(&'a self, input: &'a str) -> StepFuture<'a> {
        Box::pin(async move {
            if input == "slow" {
                let _permit = self.0.acquire().await.unwrap();
            }
            Ok(input.to_uppercase())
        })
    }
}

#[tokio::test]
async fn copied_is_accepted_while_a_slow_copy_is_in_progress() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    let gate = Arc::new(Semaphore::new(0));
    let engine = Engine::with_pipeline(Pipeline::new(vec![Box::new(GatedUpper(gate.clone()))]));
    serve(&app, engine).await.unwrap();

    let mut extension = Extension::connect(&bus).await;
    tokio::time::timeout(SOON, extension.copied("slow"))
        .await
        .expect("Copied a attendu la fin du traitement");
    tokio::time::timeout(SOON, extension.copied("abc"))
        .await
        .expect("Copied a attendu la fin du traitement précédent");
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("ABC"));

    // Le résultat de la copie lente, devenu périmé, n'est jamais appliqué.
    gate.add_permits(1);
    assert_eq!(extension.next_apply(Duration::from_millis(300)).await, None);
}
