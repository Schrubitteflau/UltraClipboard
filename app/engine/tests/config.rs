//! Chargement et validation de la config : chaque refus nomme le champ et la raison.

use engine::{Config, ConfigError, Notifications, SAMPLE_CONFIG};

fn refused(yaml: &str) -> ConfigError {
    match Config::parse(yaml) {
        Ok(_) => panic!("config acceptée à tort :\n{yaml}"),
        Err(err) => err,
    }
}

/// Le champ fautif est `field` et la raison contient `reason`.
#[track_caller]
fn assert_refused(yaml: &str, field: &str, reason: &str) {
    let err = refused(yaml);
    assert_eq!(err.field, field, "{err}");
    assert!(err.reason.contains(reason), "raison : {}", err.reason);
}

#[test]
fn the_sample_config_is_valid_and_holds_the_youtube_rule() {
    let config = Config::parse(SAMPLE_CONFIG).unwrap();
    let rules: Vec<_> = config
        .rules()
        .iter()
        .map(|r| (r.pattern(), r.pipeline()))
        .collect();
    assert_eq!(
        rules,
        [(
            r"^https?://(www\.)?(youtube\.com/watch|youtu\.be/)",
            "youtube-markdown"
        )]
    );
}

#[test]
fn rules_are_kept_in_file_order() {
    let config = Config::parse(
        "config_version: 1\npipelines:\n  a: [trim]\n  b: [upper]\nrules:\n  - match: 'z'\n    pipeline: b\n  - match: 'y'\n    pipeline: a\n",
    )
    .unwrap();
    let pipelines: Vec<_> = config.rules().iter().map(|r| r.pipeline()).collect();
    assert_eq!(pipelines, ["b", "a"]);
}

#[test]
fn a_minimal_config_has_no_rules_and_notifies_everything() {
    let config = Config::parse("config_version: 1\n").unwrap();
    assert!(config.rules().is_empty());
    assert_eq!(config.notifications(), Notifications::All);
}

#[test]
fn notification_levels_are_read() {
    for (level, want) in [
        ("all", Notifications::All),
        ("errors", Notifications::Errors),
        ("off", Notifications::Off),
    ] {
        let config =
            Config::parse(&format!("config_version: 1\nnotifications: {level}\n")).unwrap();
        assert_eq!(config.notifications(), want);
    }
}

#[test]
fn an_unknown_notification_level_is_refused() {
    assert_refused(
        "config_version: 1\nnotifications: some\n",
        "notifications",
        "all, errors ou off",
    );
}

#[test]
fn a_missing_version_is_refused() {
    assert_refused("rules: []\n", "config_version", "obligatoire");
}

#[test]
fn an_empty_file_is_refused_for_its_missing_version() {
    assert_refused("", "config_version", "obligatoire");
}

#[test]
fn an_unknown_version_is_refused() {
    assert_refused(
        "config_version: 2\n",
        "config_version",
        "version inconnue : 2",
    );
    assert_refused(
        "config_version: '1'\n",
        "config_version",
        "version inconnue",
    );
}

#[test]
fn broken_yaml_is_refused() {
    let err = refused("config_version: 1\nrules: [\n");
    assert_eq!(err.field, "");
    assert!(err.reason.starts_with("YAML invalide"), "{err}");
}

#[test]
fn an_unknown_top_level_field_is_refused() {
    assert_refused("config_version: 1\nrule: []\n", "rule", "champ inconnu");
}

#[test]
fn an_invalid_regex_is_refused() {
    assert_refused(
        "config_version: 1\npipelines:\n  p: [trim]\nrules:\n  - match: 'ok'\n    pipeline: p\n  - match: '(unclosed'\n    pipeline: p\n",
        "rules[1].match",
        "expression régulière invalide",
    );
}

#[test]
fn a_rule_naming_an_unknown_pipeline_is_refused() {
    assert_refused(
        "config_version: 1\npipelines:\n  p: [trim]\nrules:\n  - match: 'x'\n    pipeline: nope\n",
        "rules[0].pipeline",
        "Pipeline inconnu « nope »",
    );
}

#[test]
fn a_rule_without_match_or_pipeline_is_refused() {
    assert_refused(
        "config_version: 1\npipelines:\n  p: [trim]\nrules:\n  - pipeline: p\n",
        "rules[0].match",
        "obligatoire",
    );
    assert_refused(
        "config_version: 1\nrules:\n  - match: 'x'\n",
        "rules[0].pipeline",
        "obligatoire",
    );
}

#[test]
fn an_unknown_rule_field_is_refused() {
    assert_refused(
        "config_version: 1\npipelines:\n  p: [trim]\nrules:\n  - match: 'x'\n    pipeline: p\n    pipline: p\n",
        "rules[0].pipline",
        "champ inconnu",
    );
}

#[test]
fn an_unknown_transformation_is_refused() {
    assert_refused(
        "config_version: 1\npipelines:\n  p:\n    - trim\n    - shout\n",
        "pipelines.p[1]",
        "Transformation inconnue « shout »",
    );
    assert_refused(
        "config_version: 1\npipelines:\n  p:\n    - shout: {loud: true}\n",
        "pipelines.p[0]",
        "Transformation inconnue « shout »",
    );
}

#[test]
fn a_step_naming_two_transformations_is_refused() {
    assert_refused(
        "config_version: 1\npipelines:\n  p:\n    - trim:\n      upper:\n",
        "pipelines.p[0]",
        "une seule Transformation",
    );
}

#[test]
fn an_empty_pipeline_is_refused() {
    assert_refused(
        "config_version: 1\npipelines:\n  p: []\n",
        "pipelines.p",
        "Pipeline vide",
    );
}

#[test]
fn a_parameter_given_to_a_transformation_without_parameters_is_refused() {
    assert_refused(
        "config_version: 1\npipelines:\n  p:\n    - trim: {left: true}\n",
        "pipelines.p[0].trim",
        "n'accepte pas de paramètre",
    );
}

#[test]
fn a_bad_parameter_is_refused_with_its_path() {
    assert_refused(
        "config_version: 1\npipelines:\n  p:\n    - youtube: {template: [1]}\n",
        "pipelines.p[0].youtube.template",
        "attendu un texte",
    );
    assert_refused(
        "config_version: 1\npipelines:\n  p:\n    - youtube: {tempalte: x}\n",
        "pipelines.p[0].youtube.tempalte",
        "paramètre inconnu",
    );
}

#[test]
fn any_step_accepts_a_timeout_in_seconds() {
    Config::parse("config_version: 1\npipelines:\n  p:\n    - trim:\n      timeout: 10\n    - upper:\n      timeout: 0.5\n")
        .unwrap();
}

#[test]
fn a_bad_timeout_is_refused() {
    assert_refused(
        "config_version: 1\npipelines:\n  p:\n    - trim:\n      timeout: soon\n",
        "pipelines.p[0].timeout",
        "nombre de secondes",
    );
    assert_refused(
        "config_version: 1\npipelines:\n  p:\n    - trim:\n      timeout: 0\n",
        "pipelines.p[0].timeout",
        "positif",
    );
}

#[test]
fn an_unused_pipeline_is_still_validated() {
    assert_refused(
        "config_version: 1\npipelines:\n  unused: [nope]\n",
        "pipelines.unused[0]",
        "Transformation inconnue",
    );
}

#[test]
fn a_non_integer_version_shows_the_value_read() {
    let err = refused("config_version: 1.0\n");
    assert_eq!(err.field, "config_version");
    assert!(err.reason.contains("1.0"), "{err}");
    assert!(err.reason.contains("entier"), "{err}");
}

#[test]
fn a_pipeline_name_read_as_a_boolean_hints_at_quoting() {
    // `on` sans guillemets est lu comme un booléen.
    let err = refused("config_version: 1\npipelines:\n  on: [trim]\n");
    assert!(err.reason.contains("guillemets"), "{err}");
    assert_refused(
        "config_version: 1\npipelines:\n  'on': [trim]\nrules:\n  - match: 'x'\n    pipeline: on\n",
        "rules[0].pipeline",
        "guillemets",
    );
}

#[test]
fn the_quoting_hint_is_only_given_where_a_text_is_expected() {
    let err = refused("config_version: 1\npipelines:\n  p:\n    - trim:\n      timeout: yes\n");
    assert_eq!(err.field, "pipelines.p[0].timeout");
    assert!(!err.reason.contains("guillemets"), "{err}");
    let err = refused("config_version: true\n");
    assert!(!err.reason.contains("guillemets"), "{err}");
}
