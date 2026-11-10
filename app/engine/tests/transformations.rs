//! Transformations intégrées `trim`, `upper`, `lower`, sur du texte Unicode.

use engine::{Config, Engine, Outcome};

/// Moteur dont l'unique Règle applique `step` à toute copie.
fn engine_with(step: &str) -> Engine {
    let yaml = format!(
        "config_version: 1\npipelines:\n  p: [{step}]\nrules:\n  - match: ''\n    pipeline: p\n"
    );
    Engine::new(Config::parse(&yaml).unwrap())
}

async fn apply(step: &str, text: &str) -> Outcome {
    engine_with(step).on_copy(text).await
}

fn write(text: &str) -> Outcome {
    Outcome::Write(text.into())
}

#[tokio::test]
async fn trim_removes_unicode_whitespace_at_both_ends_only() {
    // Espace insécable, tabulation, retour à la ligne, espace idéographique.
    assert_eq!(
        apply("trim", "\u{a0}\t\n  deux  mots \u{3000}\n").await,
        write("deux  mots")
    );
}

#[tokio::test]
async fn upper_handles_accents_and_special_cases() {
    assert_eq!(
        apply("upper", "élève straße ǆ").await,
        write("ÉLÈVE STRASSE Ǆ")
    );
}

#[tokio::test]
async fn lower_handles_accents_and_greek() {
    assert_eq!(apply("lower", "ÉCOLE ΣΟΦΟΣ").await, write("école σοφος"));
}
