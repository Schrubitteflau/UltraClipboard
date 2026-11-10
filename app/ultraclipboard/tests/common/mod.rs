//! Banc de test : un bus de session privé, jetable, pour le contrat D-Bus.

#![allow(dead_code)] // chaque fichier de test n'utilise qu'une partie du banc

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use engine::{Config, Engine};
use futures_util::StreamExt;
use ultraclipboard::{BUS_NAME, INTERFACE, OBJECT_PATH};
use zbus::{Connection, MatchRule, MessageStream, connection};

/// Moteur dont l'unique Règle met toute copie en majuscules.
pub fn upper_everything() -> Engine {
    let yaml = "config_version: 1\npipelines:\n  shout: [upper]\nrules:\n  - match: ''\n    pipeline: shout\n";
    Engine::new(Config::parse(yaml).unwrap())
}

/// Un `dbus-daemon` privé, arrêté à la fin du test. Ne touche jamais au bus de la session.
pub struct PrivateBus {
    daemon: Child,
    pub address: String,
}

impl PrivateBus {
    pub fn start() -> Self {
        let mut daemon = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon introuvable");
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        Self {
            daemon,
            address: address.trim().to_owned(),
        }
    }

    pub async fn connect(&self) -> Connection {
        connection::Builder::address(self.address.as_str())
            .build()
            .await
            .unwrap()
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

/// Client qui joue le rôle de l'extension : appelle les méthodes, écoute `Apply`.
pub struct Extension {
    conn: Connection,
    applies: MessageStream,
}

impl Extension {
    pub async fn connect(bus: &PrivateBus) -> Self {
        let conn = bus.connect().await;
        let rule = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(INTERFACE)
            .member("Apply")
            .build()
            .unwrap();
        let applies = MessageStream::for_match_rule(rule, &conn, None)
            .await
            .unwrap();
        Self { conn, applies }
    }

    pub async fn copied(&self, text: &str) {
        self.conn
            .call_method(
                Some(BUS_NAME),
                OBJECT_PATH,
                Some(INTERFACE),
                "Copied",
                &(text,),
            )
            .await
            .unwrap();
    }

    /// Prochain texte reçu par `Apply`, ou `None` s'il n'en vient aucun dans le délai.
    pub async fn next_apply(&mut self, within: Duration) -> Option<String> {
        let msg = tokio::time::timeout(within, self.applies.next())
            .await
            .ok()??
            .unwrap();
        Some(msg.body().deserialize::<String>().unwrap())
    }
}
