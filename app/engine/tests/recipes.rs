//! Recettes publiées sur le site (`site/recettes.yaml`) : chaque exemple montré
//! sur la page passe par le moteur, avec les vrais outils du système (sh, sed,
//! awk, tr, date, bc, jq). Les deux recettes qui passent par le réseau
//! (`youtube-markdown`, `dollars-en-euros`) ne sont pas exécutées ici.

use engine::{Config, Engine, Outcome, Skip};

const RECIPES: &str = include_str!("../../../site/recettes.yaml");

/// Résultat de la copie de `text` par un moteur neuf chargé des recettes.
async fn copy(text: &str) -> Outcome {
    Engine::new(Config::parse(RECIPES).unwrap())
        .on_copy(text)
        .await
}

fn write(text: &str) -> Outcome {
    Outcome::Write(text.into())
}

#[test]
fn the_published_recipes_are_a_valid_config() {
    Config::parse(RECIPES).unwrap();
}

#[tokio::test]
async fn tracking_parameters_are_removed_from_a_link() {
    assert_eq!(
        copy("https://exemple.fr/article?utm_source=lettre&utm_campaign=promo&fbclid=IwAR0x").await,
        write("https://exemple.fr/article")
    );
    // Les autres paramètres et l'ancre restent, où que soit le pistage.
    assert_eq!(
        copy("https://exemple.fr/a?utm_source=x&id=3&fbclid=abc#titre").await,
        write("https://exemple.fr/a?id=3#titre")
    );
    assert_eq!(
        copy("https://exemple.fr/a?id=3&gclid=abc&page=2").await,
        write("https://exemple.fr/a?id=3&page=2")
    );
}

#[tokio::test]
async fn a_link_without_tracking_is_left_alone() {
    assert_eq!(
        copy("https://exemple.fr/article?id=3").await,
        Outcome::Nothing(Skip::NoRule)
    );
}

#[tokio::test]
async fn a_calculation_ending_with_an_equals_sign_becomes_its_result() {
    assert_eq!(copy("128 * 12 =").await, write("1536"));
    assert_eq!(copy("49,90 * 3 =").await, write("149,70"));
    assert_eq!(copy("(12 + 8) / 3 =").await, write("6,66"));
}

#[tokio::test]
async fn numbers_that_are_not_a_calculation_are_left_alone() {
    for text in ["2026-11-10", "06 12 34 56 78", "128 * 12", "1536", "= 12"] {
        assert_eq!(copy(text).await, Outcome::Nothing(Skip::NoRule), "{text}");
    }
}

#[tokio::test]
async fn a_time_in_an_american_zone_becomes_paris_time() {
    // Paris est à UTC+1 l'hiver et UTC+2 l'été : deux heures possibles selon
    // le jour où le test tourne.
    for (text, winter, summer) in [
        ("3pm PST", "00:00", "01:00"),
        ("9:30am EST", "15:30", "16:30"),
        ("14:00 UTC", "15:00", "16:00"),
    ] {
        let outcome = copy(text).await;
        assert!(
            [winter, summer]
                .iter()
                .any(|hour| outcome == write(&format!("{hour} (heure de Paris)"))),
            "{text} : {outcome:?}"
        );
    }
}

#[tokio::test]
async fn miles_become_kilometres() {
    assert_eq!(copy("26.2 miles").await, write("42,2 km"));
    assert_eq!(copy("5 mi").await, write("8,0 km"));
}

#[tokio::test]
async fn feet_and_inches_become_metres() {
    assert_eq!(copy("5 ft 11").await, write("1,80 m"));
    assert_eq!(copy("6 ft").await, write("1,83 m"));
}

#[tokio::test]
async fn fahrenheit_becomes_celsius() {
    assert_eq!(copy("72°F").await, write("22 °C"));
    assert_eq!(copy("14 °F").await, write("-10 °C"));
}

#[tokio::test]
async fn one_line_json_is_indented() {
    assert_eq!(
        copy(r#"{"nom":"UltraClipboard","regles":3}"#).await,
        write("{\n  \"nom\": \"UltraClipboard\",\n  \"regles\": 3\n}")
    );
}

#[tokio::test]
async fn a_markdown_link_is_not_taken_for_json() {
    assert_eq!(
        copy("[un lien](https://exemple.fr)").await,
        Outcome::Nothing(Skip::NoRule)
    );
}

#[tokio::test]
async fn a_ticket_number_becomes_a_markdown_link() {
    assert_eq!(
        copy("  PROJ-1234 ").await,
        write("[PROJ-1234](https://tracker.example.com/PROJ-1234)")
    );
}

#[tokio::test]
async fn the_prompt_is_removed_from_a_copied_command() {
    assert_eq!(copy("$ npm install").await, write("npm install"));
    assert_eq!(
        copy("$ git pull\n$ ./install.sh").await,
        write("git pull\n./install.sh")
    );
}
