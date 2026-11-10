//! Contrat D-Bus entre l'extension et l'appli, sur un bus privé.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{Extension, PrivateBus, upper_everything};
use engine::{Config, Engine};
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

/// « slow » passe par la commande `script` (texte en entrée, résultat en
/// sortie) ; toute autre copie est mise en majuscules aussitôt.
fn slow_engine(script: &str) -> Engine {
    let yaml = format!(
        "config_version: 1\npipelines:\n  slow:\n    - command: [sh, -c, '{script}']\n      timeout: 5\n  shout: [upper]\nrules:\n  - match: '^slow$'\n    pipeline: slow\n  - match: ''\n    pipeline: shout\n"
    );
    Engine::new(Config::parse(&yaml).unwrap())
}

/// Marqueur que la commande lente crée si elle va au bout ; absent au départ.
fn marker(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("contract");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    path
}

/// Commande qui, après une demi-seconde, crée `marker` puis rend le texte en
/// majuscules : sans annulation, le marqueur apparaît et « SLOW » est appliqué.
fn finishes_late(marker: &Path) -> String {
    format!("sleep 0.5; touch \"{}\"; tr a-z A-Z", marker.display())
}

#[tokio::test]
async fn copied_is_accepted_while_a_slow_copy_is_in_progress() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, slow_engine("sleep 30")).await.unwrap();

    // Bien en deçà du délai de la commande (5 s).
    let prompt = Duration::from_secs(2);
    let mut extension = Extension::connect(&bus).await;
    tokio::time::timeout(prompt, extension.copied("slow"))
        .await
        .expect("Copied a attendu la fin du traitement");
    tokio::time::timeout(prompt, extension.copied("abc"))
        .await
        .expect("Copied a attendu la fin du traitement précédent");
    assert_eq!(extension.next_apply(prompt).await.as_deref(), Some("ABC"));
}

#[tokio::test]
async fn two_quick_copies_apply_only_the_second_and_stop_the_first() {
    let marker = marker("two-quick-copies");
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, slow_engine(&finishes_late(&marker)))
        .await
        .unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.copied("slow").await;
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("ABC"));
    assert_eq!(extension.next_apply(Duration::from_secs(1)).await, None);
    assert!(
        !marker.exists(),
        "la commande de la première copie a continué"
    );
}

#[tokio::test]
async fn a_copy_without_text_stops_the_copy_in_progress() {
    let marker = marker("copy-without-text");
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, slow_engine(&finishes_late(&marker)))
        .await
        .unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.copied("slow").await;
    // Une image copiée juste après : le résultat tardif ne doit pas la remplacer.
    extension.copied_non_text().await;
    assert_eq!(extension.next_apply(Duration::from_secs(1)).await, None);
    assert!(
        !marker.exists(),
        "la commande de la copie annulée a continué"
    );
}

#[tokio::test]
async fn restore_original_applies_the_text_before_the_transformation() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, upper_everything()).await.unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("ABC"));
    // L'extension écrit « ABC » : son écho revient comme copie.
    extension.copied("ABC").await;
    extension.restore_original().await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("abc"));
    // L'écho de l'Original restauré n'est pas retransformé.
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(Duration::from_secs(1)).await, None);
}

#[tokio::test]
async fn restore_original_without_any_transformation_applies_nothing() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, upper_everything()).await.unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.restore_original().await;
    assert_eq!(extension.next_apply(Duration::from_secs(1)).await, None);
}

#[tokio::test]
async fn restore_original_stops_a_newer_copy_in_progress_and_applies_nothing() {
    let marker = marker("restore-in-progress");
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, slow_engine(&finishes_late(&marker)))
        .await
        .unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("ABC"));
    extension.copied("ABC").await;
    extension.copied("slow").await;
    // « slow » reste tel que copié : ni « abc » ni « SLOW ».
    extension.restore_original().await;
    assert_eq!(extension.next_apply(Duration::from_secs(1)).await, None);
    assert!(
        !marker.exists(),
        "la commande de la copie arrêtée a continué"
    );
}

#[tokio::test]
async fn restore_original_after_a_newer_copy_applies_nothing() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, upper_everything()).await.unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("ABC"));
    extension.copied("ABC").await;
    // Une image copiée ensuite : l'Original ne la remplace pas.
    extension.copied_non_text().await;
    extension.restore_original().await;
    assert_eq!(extension.next_apply(Duration::from_secs(1)).await, None);
}

#[tokio::test]
async fn restore_original_without_original_stops_a_copy_in_progress() {
    let marker = marker("restore-no-original");
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, slow_engine(&finishes_late(&marker)))
        .await
        .unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.copied("slow").await;
    // Aucun Original : rien n'est écrit, « slow » reste tel que copié.
    extension.restore_original().await;
    assert_eq!(extension.next_apply(Duration::from_secs(1)).await, None);
    assert!(
        !marker.exists(),
        "la commande de la copie arrêtée a continué"
    );
}

/// Unique Règle : toute copie reçoit un « ! » de plus (étape non idempotente,
/// qui révèle tout écho retraité).
fn exclaim() -> Engine {
    let yaml = "config_version: 1\npipelines:\n  exclaim:\n    - command: [sh, -c, 'cat; printf !']\nrules:\n  - match: ''\n    pipeline: exclaim\n";
    Engine::new(Config::parse(yaml).unwrap())
}

#[tokio::test]
async fn restore_original_before_the_echo_of_the_result_ends_on_the_original() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, exclaim()).await.unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("abc!"));
    // Restauration avant le retour de l'écho de « abc! », puis les deux échos.
    extension.restore_original().await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("abc"));
    extension.copied("abc!").await;
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(Duration::from_secs(1)).await, None);
}

#[tokio::test]
async fn two_quick_restores_end_on_the_original() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, exclaim()).await.unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("abc!"));
    extension.copied("abc!").await;
    extension.restore_original().await;
    extension.restore_original().await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("abc"));
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("abc"));
    extension.copied("abc").await;
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(Duration::from_secs(1)).await, None);
}
