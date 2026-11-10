//! Historique des transformations : une entrée par copie qui a déclenché un
//! Pipeline, en mémoire, plafonné à 50, conservé au rechargement de la config.

use std::time::SystemTime;

use engine::{Config, Engine, HistoryEntry, HistoryStatus};

/// `^up` passe par `shout` (majuscules), le reste rien.
const UPPER: &str = "config_version: 1\npipelines:\n  shout: [upper]\nrules:\n  - match: '^up'\n    pipeline: shout\n";

fn engine(yaml: &str) -> Engine {
    Engine::new(Config::parse(yaml).unwrap())
}

#[tokio::test]
async fn a_successful_copy_is_recorded_with_its_rule_pipeline_original_and_result() {
    let engine = engine(UPPER);
    let before = SystemTime::now();
    engine.on_copy("up and away").await;
    let after = SystemTime::now();

    let history = engine.history();
    assert_eq!(history.len(), 1);
    let HistoryEntry {
        at,
        rule,
        pipeline,
        original,
        status,
    } = &history[0];
    assert!(before <= *at && *at <= after);
    assert_eq!(rule, "^up");
    assert_eq!(pipeline, "shout");
    assert_eq!(original, "up and away");
    assert_eq!(
        *status,
        HistoryStatus::Success {
            result: "UP AND AWAY".into()
        }
    );
}

/// Le seul statut de la seule entrée.
fn only_status(engine: &Engine) -> HistoryStatus {
    let history = engine.history();
    assert_eq!(history.len(), 1, "{history:?}");
    history[0].status.clone()
}

#[tokio::test]
async fn a_failed_step_is_recorded_with_its_step_message_and_error_output() {
    let engine = engine(
        "config_version: 1\npipelines:\n  broken:\n    - trim\n    - command: [sh, -c, 'echo oups >&2; echo encore >&2; exit 3']\nrules:\n  - match: ''\n    pipeline: broken\n",
    );
    engine.on_copy(" texte ").await;

    let HistoryStatus::Failure {
        step,
        message,
        stderr,
    } = only_status(&engine)
    else {
        panic!("échec attendu");
    };
    assert_eq!(step.as_deref(), Some("command"));
    assert!(message.contains('3'), "{message}");
    assert_eq!(stderr, "oups\nencore\n");
    assert_eq!(engine.history()[0].original, " texte ");
}

/// Un résultat vide ne vide pas le presse-papier : c'est un échec du Pipeline.
#[tokio::test]
async fn an_empty_result_is_recorded_as_a_failure() {
    let engine = engine(
        "config_version: 1\npipelines:\n  blank: [trim]\nrules:\n  - match: ''\n    pipeline: blank\n",
    );
    engine.on_copy("   ").await;

    assert_eq!(
        only_status(&engine),
        HistoryStatus::Failure {
            step: None,
            message: "résultat vide".into(),
            stderr: String::new(),
        }
    );
}

/// Le Pipeline a abouti même si le presse-papier n'est pas réécrit.
#[tokio::test]
async fn an_unchanged_result_is_recorded_as_a_success() {
    let engine = engine(
        "config_version: 1\npipelines:\n  clean: [trim]\nrules:\n  - match: ''\n    pipeline: clean\n",
    );
    engine.on_copy("propre").await;

    assert_eq!(
        only_status(&engine),
        HistoryStatus::Success {
            result: "propre".into()
        }
    );
}

#[tokio::test]
async fn copies_without_a_matching_rule_are_not_recorded() {
    let engine = engine(UPPER);
    engine.on_copy("down").await;
    engine.on_copy_without_text();

    assert_eq!(engine.history(), []);
}

/// L'écho de notre propre écriture n'est pas une transformation.
#[tokio::test]
async fn the_echo_of_our_own_write_is_not_recorded() {
    let engine = engine(UPPER);
    engine.on_copy("up").await;
    engine.on_copy("UP").await;

    assert_eq!(engine.history().len(), 1);
}

#[tokio::test]
async fn a_copy_superseded_by_a_newer_one_is_recorded_as_cancelled() {
    let engine = engine(
        "config_version: 1\npipelines:\n  slow:\n    - command: [sleep, '30']\n      timeout: 5\n  shout: [upper]\nrules:\n  - match: '^slow$'\n    pipeline: slow\n  - match: ''\n    pipeline: shout\n",
    );
    let slow = tokio::spawn(engine.on_copy("slow"));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    engine.on_copy("fast").await;
    slow.await.unwrap();

    let history = engine.history();
    let summary: Vec<_> = history
        .iter()
        .map(|entry| {
            (
                entry.original.as_str(),
                entry.pipeline.as_str(),
                &entry.status,
            )
        })
        .collect();
    // Dans l'ordre des copies, la plus récente en tête.
    assert_eq!(
        summary,
        [
            (
                "fast",
                "shout",
                &HistoryStatus::Success {
                    result: "FAST".into()
                }
            ),
            ("slow", "slow", &HistoryStatus::Cancelled),
        ]
    );
}

#[tokio::test]
async fn only_the_last_50_transformations_are_kept_newest_first() {
    let engine = engine(UPPER);
    for i in 1..=53 {
        engine.on_copy(&format!("up {i}")).await;
    }

    let originals: Vec<_> = engine
        .history()
        .into_iter()
        .map(|entry| entry.original)
        .collect();
    let expected: Vec<_> = (4..=53).rev().map(|i| format!("up {i}")).collect();
    assert_eq!(originals, expected);
}

#[tokio::test]
async fn reloading_the_config_keeps_the_history() {
    let engine = engine(UPPER);
    engine.on_copy("up").await;

    engine.load("config_version: 1\n").unwrap();
    engine.load("config_version: 9\n").unwrap_err();
    engine.on_copy("up again").await;

    let history = engine.history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].original, "up");
}

#[tokio::test]
async fn each_new_entry_is_signalled() {
    let engine = engine(UPPER);
    let changes = engine.history_changes();
    engine.on_copy("down").await;
    assert!(!changes.has_changed().unwrap());

    engine.on_copy("up").await;
    assert!(changes.has_changed().unwrap());
}

/// Ce que reçoit la fenêtre.
#[tokio::test]
async fn entries_serialise_with_their_status_and_time_in_milliseconds() {
    let engine = engine(UPPER);
    engine.on_copy("up").await;

    let json = serde_json::to_value(&engine.history()[0]).unwrap();
    assert!(json["at"].as_u64().unwrap() > 1_700_000_000_000, "{json}");
    assert_eq!(
        json,
        serde_json::json!({
            "at": json["at"],
            "rule": "^up",
            "pipeline": "shout",
            "original": "up",
            "status": "success",
            "result": "UP",
        })
    );
}

/// Texte gardé par l'historique : les 65 536 premiers caractères de `text`,
/// puis le nombre de caractères retirés.
fn capped(text: &str, kept: usize, dropped: usize) -> String {
    assert_eq!(text.chars().count(), kept + dropped);
    let head: String = text.chars().take(kept).collect();
    format!("{head}\n[… {dropped} caractères de plus]")
}

/// Une copie énorme ne gonfle ni la mémoire ni ce qui est poussé à la fenêtre.
#[tokio::test]
async fn huge_texts_are_kept_capped_with_a_marker() {
    let engine = engine(
        "config_version: 1\npipelines:\n  big:\n    - command: [sh, -c, 'cat >/dev/null; head -c 8000000 /dev/zero | tr \"\\0\" b']\nrules:\n  - match: ''\n    pipeline: big\n",
    );
    // Caractères de plusieurs octets : on compte des caractères, pas des octets.
    let original = "é".repeat(65_536) + &"😀".repeat(100_000);
    engine.on_copy(&original).await;

    let entry = &engine.history()[0];
    assert_eq!(entry.original, capped(&original, 65_536, 100_000));
    let HistoryStatus::Success { result } = &entry.status else {
        panic!("succès attendu : {:?}", entry.status);
    };
    assert_eq!(*result, capped(&"b".repeat(8_000_000), 65_536, 7_934_464));
}

#[tokio::test]
async fn a_text_at_the_cap_is_kept_whole() {
    let engine = engine(UPPER);
    let original = "up".to_owned() + &"x".repeat(65_534);
    engine.on_copy(&original).await;

    assert_eq!(engine.history()[0].original, original);
}

/// Un échec avec une sortie d'erreur énorme : elle reste bornée.
#[tokio::test]
async fn a_huge_error_output_is_capped() {
    let engine = engine(
        "config_version: 1\npipelines:\n  noisy:\n    - command: [sh, -c, 'cat >&2; exit 1']\nrules:\n  - match: ''\n    pipeline: noisy\n",
    );
    let original = "e".repeat(100_000);
    engine.on_copy(&original).await;

    let HistoryStatus::Failure { stderr, .. } = &engine.history()[0].status else {
        panic!("échec attendu");
    };
    assert!(stderr.chars().count() < 65_536 + 100, "{}", stderr.len());
}

#[tokio::test]
async fn the_history_pushed_to_the_window_stays_bounded() {
    let engine = engine(UPPER);
    let big = "up".to_owned() + &"x".repeat(1_000_000);
    for _ in 0..60 {
        engine.on_copy(&big).await;
        // Le texte suivant n'est pas l'écho : on recopie autre chose entre deux.
        engine.on_copy("down").await;
    }

    let json = serde_json::to_string(&engine.history()).unwrap();
    // 50 entrées, deux textes de 64 Ki caractères (ASCII) chacune, plus un peu.
    assert!(json.len() < 50 * 2 * (65_536 + 200), "{}", json.len());
}
