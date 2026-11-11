//! Config lue au lancement : fichier d'exemple, config invalide, Règles sur le bus.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::{Extension, PrivateBus};
use engine::{Config, Engine, SAMPLE_CONFIG};
use ultraclipboard::{follow_config, serve, start_engine};

const SOON: Duration = Duration::from_secs(5);
const NONE_WITHIN: Duration = Duration::from_millis(300);

/// Dossier vide propre au test, dans le dossier temporaire de cargo.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("startup-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn a_missing_config_is_created_from_the_sample_and_loaded() {
    let path = scratch("missing")
        .join("ultraclipboard")
        .join("config.yaml");
    let engine = Engine::new(Config::default());
    let _follower = follow_config(path.clone(), engine.clone());
    let config = engine.config();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), SAMPLE_CONFIG);
    let files: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(files, ["config.yaml"], "fichier temporaire laissé derrière");
    let pipelines: Vec<_> = config.rules().iter().map(|r| r.pipeline()).collect();
    assert_eq!(pipelines, ["lien-propre"]);
    assert_eq!(engine.config_error(), None);
}

#[tokio::test]
async fn an_invalid_config_at_startup_runs_without_rules() {
    let path = scratch("invalid").join("config.yaml");
    // Valide en apparence, mais sans version : refusée.
    let yaml = "pipelines:\n  clean: [trim]\nrules:\n  - match: ''\n    pipeline: clean\n";
    std::fs::write(&path, yaml).unwrap();

    let bus = PrivateBus::start();
    let app = bus.connect().await;
    let engine = Engine::new(Config::default());
    let _follower = follow_config(path.clone(), engine.clone());
    serve(&app, engine.clone()).await.unwrap();
    let mut extension = Extension::connect(&bus).await;
    extension.copied("  a  ").await;
    assert_eq!(extension.next_apply(NONE_WITHIN).await, None);
    // L'erreur est retenue par le moteur, pour la fenêtre et les notifications.
    assert_eq!(engine.config_error().unwrap().field, "config_version");
    // Le fichier de l'utilisateur n'est pas remplacé par l'exemple.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), yaml);
}

/// Le moteur de l'appli, refusé dès le lancement : la fenêtre doit dire qu'aucune
/// config n'a été chargée, pas qu'une config vide est active.
#[tokio::test]
async fn the_app_engine_with_an_invalid_first_config_has_nothing_loaded() {
    let path = scratch("app-invalid").join("config.yaml");
    std::fs::write(&path, "rules: []\n").unwrap();

    let (engine, _follower) = start_engine(Some(path));

    let state = engine.config_state();
    assert!(!state.loaded);
    assert_eq!(state.error.unwrap().error.field, "config_version");
}

#[tokio::test]
async fn a_trim_rule_from_the_config_file_applies_on_the_bus() {
    let path = scratch("trim").join("config.yaml");
    std::fs::write(
        &path,
        "config_version: 1\npipelines:\n  clean: [trim]\nrules:\n  - match: '^\\s|\\s$'\n    pipeline: clean\n",
    )
    .unwrap();

    let bus = PrivateBus::start();
    let app = bus.connect().await;
    let engine = Engine::new(Config::default());
    let _follower = follow_config(path.clone(), engine.clone());
    serve(&app, engine).await.unwrap();
    let mut extension = Extension::connect(&bus).await;

    extension.copied("  a  ").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("a"));
    extension.copied("b").await;
    assert_eq!(extension.next_apply(NONE_WITHIN).await, None);
}

#[tokio::test]
async fn an_empty_result_is_not_applied_on_the_bus() {
    let path = scratch("empty").join("config.yaml");
    std::fs::write(
        &path,
        "config_version: 1\npipelines:\n  clean: [trim]\nrules:\n  - match: '^\\s'\n    pipeline: clean\n",
    )
    .unwrap();

    let bus = PrivateBus::start();
    let app = bus.connect().await;
    let engine = Engine::new(Config::default());
    let _follower = follow_config(path.clone(), engine.clone());
    serve(&app, engine).await.unwrap();
    let mut extension = Extension::connect(&bus).await;

    extension.copied("\n\n").await;
    assert_eq!(extension.next_apply(NONE_WITHIN).await, None);
}
