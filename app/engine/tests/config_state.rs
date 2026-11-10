//! Ce que la fenêtre montre de la config : Règles dans l'ordre, Pipelines et
//! leurs étapes, état du dernier chargement ; et le signal de chaque changement.

use std::time::Duration;

use engine::{Config, ConfigError, Engine, Refusal, RefusalKind, Step};

const TWO_RULES: &str = r#"config_version: 1
pipelines:
  yt:
    - youtube:
        template: "[{title}]({url})"
  clean:
    - trim
    - command: ["python3", "~/fix.py"]
      timeout: 5
  unused: [upper]
rules:
  - match: '^https?://youtu\.be/'
    pipeline: yt
  - match: '\d'
    pipeline: clean
"#;

#[test]
fn the_state_lists_rules_in_file_order_and_every_pipeline_with_its_steps() {
    let engine = Engine::new(Config::parse(TWO_RULES).unwrap());

    let state = serde_json::to_value(engine.config_state()).unwrap();

    assert_eq!(
        state,
        serde_json::json!({
            "rules": [
                { "pattern": r"^https?://youtu\.be/", "pipeline": "yt" },
                { "pattern": r"\d", "pipeline": "clean" },
            ],
            "pipelines": [
                { "name": "yt", "steps": [
                    { "transformation": "youtube", "params": r#"template: "[{title}]({url})""# },
                ]},
                { "name": "clean", "steps": [
                    { "transformation": "trim", "params": "" },
                    { "transformation": "command", "params": r#"["python3", "~/fix.py"], timeout: 5"# },
                ]},
                { "name": "unused", "steps": [
                    { "transformation": "upper", "params": "" },
                ]},
            ],
            "loaded": true,
            "error": null,
        })
    );
}

#[test]
fn a_refused_config_keeps_the_active_one_listed_and_carries_the_error() {
    let engine = Engine::new(Config::parse(TWO_RULES).unwrap());
    engine
        .load("config_version: 1\nrules:\n  - match: x\n    pipeline: nope\n")
        .unwrap_err();

    let state = engine.config_state();

    let patterns: Vec<_> = state.rules.iter().map(|r| r.pattern.as_str()).collect();
    assert_eq!(patterns, [r"^https?://youtu\.be/", r"\d"]);
    assert!(state.loaded);
    assert_eq!(
        state.error,
        Some(Refusal {
            kind: RefusalKind::Invalid,
            error: ConfigError {
                field: "rules[0].pipeline".into(),
                reason: "Pipeline inconnu « nope »".into(),
            },
        })
    );
}

#[test]
fn a_refused_first_config_leaves_nothing_loaded() {
    let engine = Engine::default();
    engine.load("config_version: 2\n").unwrap_err();

    let state = engine.config_state();

    assert!(!state.loaded);
    assert!(state.rules.is_empty());
    assert_eq!(state.error.unwrap().kind, RefusalKind::Invalid);
}

#[test]
fn an_unreadable_file_is_told_apart_from_a_refused_content() {
    let engine = Engine::new(Config::parse(TWO_RULES).unwrap());
    let missing = ConfigError {
        field: String::new(),
        reason: "fichier absent".into(),
    };
    engine.reject_config(missing.clone());

    let state = engine.config_state();

    assert!(state.loaded);
    assert_eq!(state.rules.len(), 2);
    assert_eq!(
        state.error,
        Some(Refusal {
            kind: RefusalKind::Unreadable,
            error: missing,
        })
    );
}

#[test]
fn a_successful_load_after_an_empty_start_counts_as_loaded() {
    let engine = Engine::default();
    assert!(!engine.config_state().loaded);
    engine.load(TWO_RULES).unwrap();
    assert!(engine.config_state().loaded);
}

/// Étapes du seul Pipeline `p` de cette config.
fn steps_of(pipeline_yaml: &str) -> Vec<Step> {
    let yaml = format!("config_version: 1\npipelines:\n  p:\n{pipeline_yaml}");
    let engine = Engine::new(Config::parse(&yaml).unwrap());
    engine.config_state().pipelines.remove(0).steps
}

#[test]
fn step_parameters_are_shown_as_written() {
    let steps = steps_of("    - command: ['sed', 's/\\d+//']\n");
    assert_eq!(steps[0].params, r#"["sed", "s/\d+//"]"#);

    // Émoji composé (ZWJ), sélecteur de variante, accents décomposés (NFD).
    let written = "👩‍💻 ❤️ e\u{301}te\u{301}";
    let steps = steps_of(&format!(
        "    - youtube:\n        template: \"{written}\"\n"
    ));
    assert_eq!(steps[0].params, format!("template: \"{written}\""));
}

#[test]
fn control_and_bidi_characters_in_parameters_are_escaped() {
    let steps =
        steps_of("    - youtube:\n        template: \"a\\tb\\u202Ec\\u061Cd\\u2028e\\u2029f\"\n");
    assert_eq!(
        steps[0].params,
        r#"template: "a\tb\u{202e}c\u{61c}d\u{2028}e\u{2029}f""#
    );
}

#[test]
fn an_integer_and_a_text_key_naming_the_same_pipeline_are_refused() {
    let err = Config::parse("config_version: 1\npipelines:\n  1: [trim]\n  \"1\": [upper]\n")
        .err()
        .expect("deux Pipelines du même nom acceptés");
    assert_eq!(err.field, "pipelines.1");
    assert!(err.reason.contains("même nom"), "raison : {}", err.reason);
}

/// Attend le prochain signal de changement, une seconde au plus.
async fn changed(rx: &mut tokio::sync::watch::Receiver<()>) -> bool {
    tokio::time::timeout(Duration::from_secs(1), rx.changed())
        .await
        .is_ok_and(|r| r.is_ok())
}

#[tokio::test]
async fn every_load_accepted_or_refused_signals_a_change() {
    let engine = Engine::new(Config::default());
    let mut changes = engine.config_changes();

    engine.load(TWO_RULES).unwrap();
    assert!(changed(&mut changes).await, "config acceptée");

    engine.load("config_version: 2\n").unwrap_err();
    assert!(changed(&mut changes).await, "config refusée");

    engine.reject_config(ConfigError {
        field: String::new(),
        reason: "fichier absent".into(),
    });
    assert!(changed(&mut changes).await, "fichier illisible");

    assert!(!changed(&mut changes).await, "rien de plus");
}
