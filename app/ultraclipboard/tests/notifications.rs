//! Notifications du bureau : l'appli montre, via le service de notifications
//! standard (`org.freedesktop.Notifications`), ce que le moteur décide de notifier.
//! Le service est un faux, sur un bus privé.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{Desktop, Extension, PrivateBus};
use engine::Engine;
use ultraclipboard::{Started, start_app};

const SOON: Duration = Duration::from_secs(5);
const NONE_WITHIN: Duration = Duration::from_millis(300);

const RULES: &str = "config_version: 1\npipelines:\n  shout: [upper]\n  broken:\n    - command: [sh, -c, 'echo détail >&2; echo \"<cause & effet>\" >&2; exit 3']\n  nul:\n    - command: [sh, -c, 'printf \"bad\\\\000byte\" >&2; exit 1']\nrules:\n  - match: '^ok'\n    pipeline: shout\n  - match: '^ko'\n    pipeline: broken\n  - match: '^nul'\n    pipeline: nul\n";

/// Dossier vide propre au test, dans le dossier temporaire de cargo.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("notifications-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct App {
    engine: Engine,
    desktop: Desktop,
    extension: Extension,
    _started: Started,
    _bus: PrivateBus,
}

/// Lance l'appli comme le fait le binaire, sur ce fichier de config.
async fn start(path: &Path, yaml: &str) -> App {
    let bus = PrivateBus::start();
    let desktop = Desktop::start(&bus).await;
    start_with(bus, desktop, path, yaml).await
}

async fn start_with(bus: PrivateBus, desktop: Desktop, path: &Path, yaml: &str) -> App {
    std::fs::write(path, yaml).unwrap();
    let started = start_app(Some(path.to_owned()), bus.connector())
        .await
        .unwrap();
    let extension = Extension::connect(&bus).await;
    App {
        engine: started.engine.clone(),
        desktop,
        extension,
        _bus: bus,
        _started: started,
    }
}

#[tokio::test]
async fn a_transformed_copy_shows_its_pipeline() {
    let path = scratch("success").join("config.yaml");
    let mut app = start(&path, RULES).await;
    app.extension.copied("ok").await;
    let shown = app.desktop.next(SOON).await.unwrap();
    assert_eq!(shown.app_name, "UltraClipboard");
    assert_eq!(shown.summary, "✓ shout");
}

#[tokio::test]
async fn a_failed_copy_shows_its_pipeline_and_the_reason() {
    let path = scratch("failure").join("config.yaml");
    let mut app = start(&path, RULES).await;
    app.extension.copied("ko").await;
    let shown = app.desktop.next(SOON).await.unwrap();
    assert_eq!(shown.summary, "✗ broken");
    assert!(shown.body.starts_with("command : "), "{}", shown.body);
    assert!(
        shown.body.contains('3'),
        "code de sortie absent : {}",
        shown.body
    );
}

#[tokio::test]
async fn an_invalid_config_at_startup_is_shown() {
    let path = scratch("startup").join("config.yaml");
    let mut app = start(&path, "rules: []\n").await;
    let shown = app.desktop.next(SOON).await.unwrap();
    assert_eq!(shown.summary, "Config invalide");
    assert!(shown.body.contains("config_version"), "{}", shown.body);
}

#[tokio::test]
async fn every_save_of_an_invalid_config_is_shown() {
    let path = scratch("saves").join("config.yaml");
    let mut app = start(&path, RULES).await;
    let broken = "config_version: 1\nrules:\n  - match: ''\n    pipeline: absent\n";
    for _ in 0..2 {
        std::fs::write(&path, broken).unwrap();
        let shown = app.desktop.next(SOON).await.unwrap();
        assert_eq!(shown.summary, "Config invalide");
        assert!(shown.body.contains("rules[0].pipeline"), "{}", shown.body);
    }
}

#[tokio::test]
async fn a_muted_session_shows_nothing() {
    let path = scratch("muted").join("config.yaml");
    let mut app = start(&path, RULES).await;
    app.engine.set_notifications_muted(true);
    app.extension.copied("ok").await;
    app.extension.copied("ko").await;
    assert_eq!(app.desktop.next(NONE_WITHIN).await, None);
}

#[tokio::test]
async fn a_nul_byte_is_left_out_and_the_service_keeps_running() {
    let path = scratch("nul").join("config.yaml");
    let mut app = start(&path, RULES).await;
    app.extension.copied("nul").await;
    let shown = app.desktop.next(SOON).await.unwrap();
    assert_eq!(shown.summary, "✗ nul");
    assert!(shown.body.ends_with("\nbadbyte"), "{:?}", shown.body);
    app.extension.copied("ok").await;
    assert_eq!(app.extension.next_apply(SOON).await.as_deref(), Some("OK"));
}

#[tokio::test]
async fn a_new_success_replaces_the_previous_success_only() {
    let path = scratch("replace-success").join("config.yaml");
    let mut app = start(&path, RULES).await;
    app.extension.copied("ok1").await;
    let first = app.desktop.next(SOON).await.unwrap();
    assert_eq!(first.replaces_id, 0);
    app.extension.copied("ko").await;
    let failure = app.desktop.next(SOON).await.unwrap();
    assert_eq!(failure.replaces_id, 0, "un échec ne remplace rien");
    app.extension.copied("ok2").await;
    let second = app.desktop.next(SOON).await.unwrap();
    // Le faux service numérote ses bulles à partir de 1 : la première est la 1.
    assert_eq!(second.summary, "✓ shout");
    assert_eq!(second.replaces_id, 1);
    app.extension.copied("ko").await;
    assert_eq!(app.desktop.next(SOON).await.unwrap().replaces_id, 0);
}

#[tokio::test]
async fn a_new_invalid_config_replaces_the_previous_one() {
    let path = scratch("replace-config").join("config.yaml");
    let mut app = start(&path, "rules: []\n").await;
    let first = app.desktop.next(SOON).await.unwrap();
    assert_eq!(
        (first.summary.as_str(), first.replaces_id),
        ("Config invalide", 0)
    );
    std::fs::write(&path, "rules: [1]\n").unwrap();
    let second = app.desktop.next(SOON).await.unwrap();
    assert_eq!(
        (second.summary.as_str(), second.replaces_id),
        ("Config invalide", 1)
    );
}

#[tokio::test]
async fn a_stuck_notification_service_does_not_hold_back_the_next_ones() {
    let path = scratch("stuck").join("config.yaml");
    let bus = PrivateBus::start();
    let desktop = Desktop::stuck_once(&bus).await;
    let mut app = start_with(bus, desktop, &path, RULES).await;
    app.extension.copied("ok").await;
    app.extension.copied("ko").await;
    let shown = app.desktop.next(SOON).await.unwrap();
    assert_eq!(shown.summary, "✗ broken");
}

#[tokio::test]
async fn an_empty_result_is_shown_as_a_failure() {
    let path = scratch("empty").join("config.yaml");
    let yaml = "config_version: 1\npipelines:\n  clean: [trim]\nrules:\n  - match: ''\n    pipeline: clean\n";
    let mut app = start(&path, yaml).await;
    app.extension.copied("  ").await;
    let shown = app.desktop.next(SOON).await.unwrap();
    assert_eq!(shown.summary, "✗ clean");
    assert!(shown.body.contains("résultat vide"), "{}", shown.body);
}

#[tokio::test]
async fn a_long_line_is_cut_at_300_characters() {
    let path = scratch("long-line").join("config.yaml");
    let yaml = "config_version: 1\npipelines:\n  long:\n    - command: [sh, -c, 'printf \"%01000d\" 0 >&2; exit 1']\nrules:\n  - match: ''\n    pipeline: long\n";
    let mut app = start(&path, yaml).await;
    app.extension.copied("x").await;
    let shown = app.desktop.next(SOON).await.unwrap();
    let last = shown.body.lines().last().unwrap();
    assert_eq!(last, format!("{}…", "0".repeat(300)));
}
