//! Rafale de `Copied` : l'ordre d'arrivée sur le bus est l'ordre des copies.

mod common;
use common::{Extension, PrivateBus, upper_everything};
use std::time::Duration;
use ultraclipboard::serve;

/// Avant le correctif `spawn = false`, environ 9 rafales sur 10 échouaient.
const ROUNDS: usize = 50;

/// Une rafale de copies envoyées sans attendre la réponse (comme l'extension) :
/// le dernier Apply doit être celui de la dernière copie.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn burst_last_copy_wins() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, upper_everything()).await.unwrap();
    let mut ext = Extension::connect(&bus).await;
    let sender = bus.connect().await;
    let mut failures = 0;
    for round in 0..ROUNDS {
        let texts: Vec<String> = (0..5).map(|i| format!("r{round}c{i}")).collect();
        // Envoi strictement ordonné, sans attendre les réponses (comme l'extension, Gio async).
        for t in &texts {
            let msg = zbus::Message::method_call(ultraclipboard::OBJECT_PATH, "Copied")
                .destination(ultraclipboard::BUS_NAME)
                .interface(ultraclipboard::INTERFACE)
                .build(&(t.as_str(),))
                .unwrap();
            sender.send(&msg).await.unwrap();
        }
        let mut last = None;
        while let Some(a) = ext.next_apply(Duration::from_millis(100)).await {
            last = Some(a);
        }
        let want = texts.last().unwrap().to_uppercase();
        if last.as_deref() != Some(want.as_str()) {
            failures += 1;
            if failures <= 5 {
                eprintln!("round {round}: last Apply {last:?}, attendu {want}");
            }
        }
    }
    assert_eq!(
        failures, 0,
        "{failures}/{ROUNDS} rafales : la dernière copie n'a pas gagné"
    );
}
