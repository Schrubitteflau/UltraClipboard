//! Aucun contenu copié dans les journaux du service : sa sortie d'erreur part
//! dans le journal systemd, persistant (US43).

mod common;

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{Extension, PrivateBus};
use engine::{Config, Engine};
use ultraclipboard::serve;

/// Adresse du bus privé sur lequel le processus enfant sert le moteur.
const CHILD_BUS: &str = "ULTRACLIPBOARD_TEST_CHILD_BUS";
const READY: &str = "service prêt";

/// Processus enfant de [`a_failing_step_never_logs_the_copied_text`] : sert,
/// sur le bus donné, un moteur dont la commande recopie son entrée sur la sortie
/// d'erreur puis échoue. Ne fait rien lancé seul.
#[tokio::test]
#[ignore = "processus enfant d'un autre test"]
async fn child_serves_a_command_that_echoes_its_input_to_stderr() {
    let Ok(address) = std::env::var(CHILD_BUS) else {
        return;
    };
    let yaml = "config_version: 1\npipelines:\n  leaky:\n    - command: [sh, -c, 'cat >&2; exit 1']\nrules:\n  - match: ''\n    pipeline: leaky\n";
    let conn = zbus::connection::Builder::address(address.as_str())
        .build()
        .await
        .unwrap();
    serve(&conn, Engine::new(Config::parse(yaml).unwrap()))
        .await
        .unwrap();
    eprintln!("{READY}");
    std::future::pending::<()>().await;
}

#[tokio::test]
async fn a_failing_step_never_logs_the_copied_text() {
    let bus = PrivateBus::start();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "child_serves_a_command_that_echoes_its_input_to_stderr",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env(CHILD_BUS, &bus.address)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let logged = Arc::new(Mutex::new(String::new()));
    let reader = {
        let logged = Arc::clone(&logged);
        let stderr = child.stderr.take().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let mut logged = logged.lock().unwrap();
                logged.push_str(&line.unwrap());
                logged.push('\n');
            }
        })
    };
    let until = async |what: &str| {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !logged.lock().unwrap().contains(what) {
            assert!(
                Instant::now() < deadline,
                "« {what} » attendu ; journal : {}",
                logged.lock().unwrap()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };

    until(READY).await;
    let extension = Extension::connect(&bus).await;
    extension.copied("mot-de-passe-7f3a91").await;
    until("échec").await;
    // Ce qui suivrait la ligne d'échec a le temps d'arriver.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = child.kill();
    let _ = child.wait();
    reader.join().unwrap();

    let logged = logged.lock().unwrap();
    assert!(logged.contains("command"), "l'étape en cause : {logged}");
    assert!(
        !logged.contains("mot-de-passe-7f3a91"),
        "texte copié dans le journal : {logged}"
    );
}
