//! Choix de la Règle et exécution de son Pipeline, à partir d'une config YAML.

use engine::{Config, Engine, Outcome, Skip};

fn engine(yaml: &str) -> Engine {
    Engine::new(Config::parse(yaml).expect("config valide"))
}

const TWO_RULES: &str = r#"
config_version: 1
pipelines:
  shout:
    - upper
  whisper:
    - lower
rules:
  - match: '^https://example\.com/'
    pipeline: whisper
  - match: '^https?://'
    pipeline: shout
"#;

#[tokio::test]
async fn the_first_matching_rule_wins() {
    let engine = engine(TWO_RULES);
    assert_eq!(
        engine.on_copy("https://example.com/ABC").await,
        Outcome::Write("https://example.com/abc".into())
    );
    assert_eq!(
        engine.on_copy("https://other.org/abc").await,
        Outcome::Write("HTTPS://OTHER.ORG/ABC".into())
    );
}

#[tokio::test]
async fn without_a_matching_rule_the_clipboard_is_left_alone() {
    let engine = engine(TWO_RULES);
    assert_eq!(
        engine.on_copy("just some text").await,
        Outcome::Nothing(Skip::NoRule)
    );
}

#[tokio::test]
async fn without_any_rule_nothing_is_transformed() {
    let engine = Engine::new(Config::default());
    assert_eq!(engine.on_copy("abc").await, Outcome::Nothing(Skip::NoRule));
}

#[tokio::test]
async fn match_is_searched_anywhere_in_the_text() {
    let engine = engine(
        r#"
config_version: 1
pipelines:
  shout: [upper]
rules:
  - match: 'needle'
    pipeline: shout
"#,
    );
    assert_eq!(
        engine.on_copy("hay needle hay").await,
        Outcome::Write("HAY NEEDLE HAY".into())
    );
}

#[tokio::test]
async fn a_pipeline_applies_its_steps_in_order() {
    let engine = engine(
        r#"
config_version: 1
pipelines:
  clean:
    - trim
    - upper
rules:
  - match: ''
    pipeline: clean
"#,
    );
    assert_eq!(
        engine.on_copy("  été  ").await,
        Outcome::Write("ÉTÉ".into())
    );
}

#[tokio::test]
async fn an_empty_result_leaves_the_clipboard_alone() {
    let engine = engine(
        r#"
config_version: 1
pipelines:
  clean: [trim]
rules:
  - match: '^\s'
    pipeline: clean
"#,
    );
    assert_eq!(
        engine.on_copy("\n\n").await,
        Outcome::Nothing(Skip::EmptyResult)
    );
}

#[tokio::test]
async fn a_failing_step_discards_the_earlier_steps_result() {
    // `youtube` échoue tant que le ticket #7 n'est pas livré.
    let engine = engine(
        r#"
config_version: 1
pipelines:
  p: [upper, youtube]
rules:
  - match: ''
    pipeline: p
"#,
    );
    match engine.on_copy("abc").await {
        Outcome::Failed(failure) => assert_eq!(failure.step, "youtube"),
        other => panic!("attendu un échec de youtube, obtenu {other:?}"),
    }
}
