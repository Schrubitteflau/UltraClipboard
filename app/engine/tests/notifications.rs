//! Politique de notification : le moteur décide, pour chaque issue, s'il faut
//! notifier, selon le niveau de la config et la bascule de session.

use engine::{Config, ConfigError, Engine, Notification, Outcome};

/// Config dont la Règle « shout » met en majuscules ce qui commence par `ok`, et
/// la Règle « broken » fait échouer ce qui commence par `ko`.
fn config(level: &str) -> String {
    format!(
        "config_version: 1\nnotifications: {level}\npipelines:\n  shout: [upper]\n  broken:\n    - command: [sh, -c, 'echo boom >&2; exit 3']\nrules:\n  - match: '^ok'\n    pipeline: shout\n  - match: '^ko'\n    pipeline: broken\n"
    )
}

fn engine(level: &str) -> Engine {
    Engine::new(Config::parse(&config(level)).unwrap())
}

/// Notifications émises jusqu'ici, dans l'ordre.
fn drain(rx: &mut tokio::sync::broadcast::Receiver<Notification>) -> Vec<Notification> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

#[tokio::test]
async fn a_written_result_notifies_its_pipeline() {
    let engine = engine("all");
    let mut notices = engine.subscribe_notifications();
    engine.on_copy("ok").await;
    assert_eq!(
        drain(&mut notices),
        [Notification::Succeeded {
            pipeline: "shout".into()
        }]
    );
}

#[tokio::test]
async fn a_failed_pipeline_notifies_its_pipeline_and_the_reason() {
    let engine = engine("all");
    let mut notices = engine.subscribe_notifications();
    engine.on_copy("ko").await;
    let notified = drain(&mut notices);
    let [Notification::Failed { pipeline, failure }] = notified.as_slice() else {
        panic!("attendu une notification d'échec");
    };
    assert_eq!(pipeline, "broken");
    assert_eq!(failure.step, "command");
    assert_eq!(failure.stderr, "boom\n");
}

#[test]
fn every_refused_config_notifies_its_error() {
    let engine = engine("all");
    let mut notices = engine.subscribe_notifications();
    let err = engine.load("config_version: 2\n").unwrap_err();
    // Chaque enregistrement d'un fichier invalide est signalé de nouveau.
    engine.load("config_version: 2\n").unwrap_err();
    assert_eq!(
        drain(&mut notices),
        [
            Notification::ConfigRejected(err.clone()),
            Notification::ConfigRejected(err)
        ]
    );
}

#[test]
fn an_unreadable_config_notifies_its_error() {
    let engine = engine("all");
    let mut notices = engine.subscribe_notifications();
    let err = ConfigError {
        field: String::new(),
        reason: "fichier absent".into(),
    };
    engine.reject_config(err.clone());
    assert_eq!(drain(&mut notices), [Notification::ConfigRejected(err)]);
}

#[test]
fn a_valid_config_notifies_nothing() {
    let engine = engine("all");
    let mut notices = engine.subscribe_notifications();
    engine.load(&config("all")).unwrap();
    assert_eq!(drain(&mut notices), []);
}

/// Ce que chaque issue notifie : (succès, échec, config refusée).
async fn notified(engine: &Engine) -> (bool, bool, bool) {
    let mut notices = engine.subscribe_notifications();
    engine.on_copy("ok").await;
    let success = !drain(&mut notices).is_empty();
    engine.on_copy("ko").await;
    let failure = !drain(&mut notices).is_empty();
    engine.load("config_version: 2\n").unwrap_err();
    let rejected = !drain(&mut notices).is_empty();
    (success, failure, rejected)
}

#[tokio::test]
async fn the_level_chooses_what_is_notified() {
    assert_eq!(notified(&engine("all")).await, (true, true, true));
    assert_eq!(notified(&engine("errors")).await, (false, true, true));
    assert_eq!(notified(&engine("off")).await, (false, false, false));
}

#[tokio::test]
async fn muting_the_session_silences_every_level() {
    for level in ["all", "errors", "off"] {
        let engine = engine(level);
        engine.set_notifications_muted(true);
        assert!(engine.notifications_muted());
        assert_eq!(notified(&engine).await, (false, false, false), "{level}");
    }
}

#[tokio::test]
async fn unmuting_the_session_restores_the_level() {
    for (level, expected) in [
        ("all", (true, true, true)),
        ("errors", (false, true, true)),
        ("off", (false, false, false)),
    ] {
        let engine = engine(level);
        engine.set_notifications_muted(true);
        engine.set_notifications_muted(false);
        assert!(!engine.notifications_muted());
        assert_eq!(notified(&engine).await, expected, "{level}");
    }
}

#[tokio::test]
async fn the_session_is_not_muted_at_start() {
    assert!(!engine("all").notifications_muted());
}

#[tokio::test]
async fn a_reloaded_level_applies_to_the_next_copies() {
    let engine = engine("all");
    engine.load(&config("off")).unwrap();
    assert_eq!(notified(&engine).await, (false, false, false));
}

#[tokio::test]
async fn copies_that_change_nothing_notify_nothing() {
    let engine = engine("all");
    let mut notices = engine.subscribe_notifications();
    // Aucune Règle.
    engine.on_copy("rien").await;
    assert_eq!(drain(&mut notices), []);
    // Écho de notre propre écriture.
    engine.on_copy("ok").await;
    drain(&mut notices);
    engine.on_copy("OK").await;
    assert_eq!(drain(&mut notices), []);
    // Résultat identique au texte copié.
    let same = "config_version: 1
pipelines:
  clean: [trim]
rules:
  - match: ''
    pipeline: clean
";
    engine.load(same).unwrap();
    engine.on_copy("propre").await;
    assert_eq!(drain(&mut notices), []);
}

#[tokio::test]
async fn a_cancelled_copy_notifies_nothing() {
    let yaml = "config_version: 1\npipelines:\n  slow:\n    - command: [sleep, '5']\nrules:\n  - match: '^slow$'\n    pipeline: slow\n";
    let engine = Engine::new(Config::parse(yaml).unwrap());
    let mut notices = engine.subscribe_notifications();
    let slow = engine.on_copy("slow");
    engine.on_copy_without_text();
    slow.await;
    assert_eq!(drain(&mut notices), []);
}

#[tokio::test]
async fn an_empty_result_notifies_a_failure_of_its_pipeline() {
    let yaml = "config_version: 1\nnotifications: errors\npipelines:\n  clean: [trim]\nrules:\n  - match: ''\n    pipeline: clean\n";
    let engine = Engine::new(Config::parse(yaml).unwrap());
    let mut notices = engine.subscribe_notifications();
    engine.on_copy("   ").await;
    assert_eq!(
        drain(&mut notices),
        [Notification::EmptyResult {
            pipeline: "clean".into()
        }]
    );
    engine.load(&yaml.replace("errors", "off")).unwrap();
    engine.on_copy("  \n").await;
    assert_eq!(drain(&mut notices), []);
}

#[tokio::test]
async fn the_cause_of_a_failure_is_the_last_line_of_its_error_output() {
    let engine = engine("all");
    let mut notices = engine.subscribe_notifications();
    engine.on_copy("ko").await;
    let notified = drain(&mut notices);
    let [Notification::Failed { failure, .. }] = notified.as_slice() else {
        panic!("attendu une notification d'échec");
    };
    assert_eq!(failure.cause(), Some("boom"));
}

#[tokio::test]
async fn the_cause_of_a_cut_error_output_is_its_last_kept_line() {
    // 100 000 octets de « ligne N » : coupés à 64 Ko, au milieu d'une ligne.
    let yaml = "config_version: 1\npipelines:\n  noisy:\n    - command: [sh, -c, 'i=0; while [ $i -lt 20000 ]; do echo \"l$i\"; i=$((i+1)); done >&2; exit 1']\nrules:\n  - match: ''\n    pipeline: noisy\n";
    let engine = Engine::new(Config::parse(yaml).unwrap());
    let mut notices = engine.subscribe_notifications();
    engine.on_copy("x").await;
    let notified = drain(&mut notices);
    let [Notification::Failed { failure, .. }] = notified.as_slice() else {
        panic!("attendu une notification d'échec");
    };
    let cause = failure.cause().unwrap();
    assert!(cause.starts_with('l'), "{cause}");
    assert!(!cause.contains("coupée"), "{cause}");
}

#[tokio::test]
async fn a_failure_without_error_output_has_no_cause() {
    let yaml = "config_version: 1\npipelines:\n  quiet:\n    - command: [sh, -c, 'exit 1']\nrules:\n  - match: ''\n    pipeline: quiet\n";
    let engine = Engine::new(Config::parse(yaml).unwrap());
    let Outcome::Failed(failure) = engine.on_copy("x").await else {
        panic!("attendu un échec");
    };
    assert_eq!(failure.cause(), None);
}

#[tokio::test]
async fn a_reapply_notifies_like_a_copy() {
    let engine = engine("all");
    engine.set_paused(true);
    let mut notices = engine.subscribe_notifications();
    engine.reapply("ok").await;
    assert_eq!(
        drain(&mut notices),
        [Notification::Succeeded {
            pipeline: "shout".into()
        }]
    );
}

#[tokio::test]
async fn a_copy_made_while_paused_notifies_nothing() {
    let engine = engine("all");
    engine.set_paused(true);
    let mut notices = engine.subscribe_notifications();
    engine.on_copy("ok").await;
    engine.on_copy("ko").await;
    assert_eq!(drain(&mut notices), []);
}
