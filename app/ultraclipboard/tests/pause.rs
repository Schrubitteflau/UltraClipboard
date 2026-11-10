//! Contrat D-Bus de la pause et de la réapplication, sur un bus privé.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::{Extension, PrivateBus, upper_everything};
use ultraclipboard::serve;

const SOON: Duration = Duration::from_secs(5);
const NONE_WITHIN: Duration = Duration::from_secs(1);

#[tokio::test]
async fn a_copy_while_paused_produces_no_apply() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, upper_everything()).await.unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.set_paused(true).await;
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(NONE_WITHIN).await, None);
    // Reprise : les copies sont de nouveau transformées.
    extension.set_paused(false).await;
    extension.copied("def").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("DEF"));
}

#[tokio::test]
async fn reapply_produces_apply() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, upper_everything()).await.unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.reapply("abc").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("ABC"));
}

#[tokio::test]
async fn reapply_produces_apply_while_paused() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, upper_everything()).await.unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.set_paused(true).await;
    // La pause est bien là : la même copie ne donne rien.
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(NONE_WITHIN).await, None);
    extension.reapply("abc").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("ABC"));
}

/// Unique Règle : toute copie reçoit un « ! » de plus (étape non idempotente,
/// qui révèle tout écho retraité).
fn exclaim() -> engine::Engine {
    let yaml = "config_version: 1\npipelines:\n  exclaim:\n    - command: [sh, -c, 'cat; printf !']\nrules:\n  - match: ''\n    pipeline: exclaim\n";
    engine::Engine::new(engine::Config::parse(yaml).unwrap())
}

#[tokio::test]
async fn reapply_transforms_our_own_write() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, exclaim()).await.unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("abc!"));
    // « abc! » est notre écriture et son écho est encore attendu : réappliquée,
    // elle est transformée quand même.
    extension.reapply("abc!").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("abc!!"));
    // Les deux échos arrivent ensuite : aucun n'est retraité.
    extension.copied("abc!").await;
    extension.copied("abc!!").await;
    assert_eq!(extension.next_apply(NONE_WITHIN).await, None);
}

#[tokio::test]
async fn pausing_stops_the_copy_in_progress() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("contract");
    std::fs::create_dir_all(&dir).unwrap();
    let pidfile = dir.join("pause-in-progress.pid");
    let _ = std::fs::remove_file(&pidfile);
    let yaml = format!(
        "config_version: 1\npipelines:\n  slow:\n    - command: [sh, -c, 'echo $$ > \"$0\"; sleep 1; echo DONE', '{}']\n      timeout: 5\nrules:\n  - match: ''\n    pipeline: slow\n",
        pidfile.display()
    );
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(
        &app,
        engine::Engine::new(engine::Config::parse(&yaml).unwrap()),
    )
    .await
    .unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.copied("slow").await;
    let pid = wait_for_pid(&pidfile).await;
    extension.set_paused(true).await;
    // La commande aurait fini au bout d'une seconde : rien n'est écrit.
    assert_eq!(extension.next_apply(Duration::from_secs(2)).await, None);
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "la commande de la copie arrêtée vit encore"
    );
    std::fs::remove_file(&pidfile).unwrap();
}

/// Attend (5 s au plus) le numéro de processus écrit dans `pidfile`.
async fn wait_for_pid(pidfile: &std::path::Path) -> u32 {
    for _ in 0..500 {
        if let Ok(text) = std::fs::read_to_string(pidfile)
            && text.ends_with('\n')
        {
            return text.trim().parse().unwrap();
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("aucun numéro de processus dans {}", pidfile.display());
}

#[tokio::test]
async fn restore_original_applies_while_paused() {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    serve(&app, upper_everything()).await.unwrap();

    let mut extension = Extension::connect(&bus).await;
    extension.copied("abc").await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("ABC"));
    extension.copied("ABC").await;
    extension.set_paused(true).await;
    extension.restore_original().await;
    assert_eq!(extension.next_apply(SOON).await.as_deref(), Some("abc"));
}
