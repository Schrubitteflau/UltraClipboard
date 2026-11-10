//! Rafale espacée : entre deux `Copied`, une pause aléatoire de 0 à 500 µs
//! laisse aux copies précédentes le temps de finir. Le dernier Apply doit
//! rester celui de la dernière copie (US12) : chaque écriture part dans l'ordre
//! où le moteur l'a décidée, jamais après celle d'une copie plus récente.

mod common;
use common::{Extension, PrivateBus, upper_everything};
use std::time::Duration;
use ultraclipboard::serve;

const ROUNDS: usize = 100;
const MAX_GAP_US: u64 = 500;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn spaced_burst_last_copy_wins() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, upper_everything()).await.unwrap();
    let mut ext = Extension::connect(&bus).await;
    let sender = bus.connect().await;
    let mut failures = vec![];
    // xorshift64 à graine fixe : des pauses variées, sans dépendance.
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    for round in 0..ROUNDS {
        let texts: Vec<String> = (0..5).map(|i| format!("r{round}c{i}")).collect();
        for t in &texts {
            let msg = zbus::Message::method_call(ultraclipboard::OBJECT_PATH, "Copied")
                .destination(ultraclipboard::BUS_NAME)
                .interface(ultraclipboard::INTERFACE)
                .build(&(t.as_str(),))
                .unwrap();
            sender.send(&msg).await.unwrap();
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            // Pause bloquante : plus fine que la minuterie de tokio.
            std::thread::sleep(Duration::from_micros(seed % (MAX_GAP_US + 1)));
        }
        let mut applies = vec![];
        while let Some(a) = ext.next_apply(Duration::from_millis(100)).await {
            applies.push(a);
        }
        let want = texts.last().unwrap().to_uppercase();
        if applies.last() != Some(&want) {
            eprintln!("round {round}: Apply {applies:?}, attendu en dernier {want}");
            failures.push(round);
        }
    }
    assert!(
        failures.is_empty(),
        "{}/{ROUNDS} rafales où la dernière copie n'a pas gagné : {failures:?}",
        failures.len()
    );
}
