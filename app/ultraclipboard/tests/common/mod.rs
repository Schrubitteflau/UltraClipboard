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

/// Numéro du prochain bus privé de ce processus de test.
static NEXT_BUS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Un `dbus-daemon` privé, arrêté à la fin du test. Ne touche jamais au bus de la session.
pub struct PrivateBus {
    daemon: Child,
    pub address: String,
}

impl PrivateBus {
    pub fn start() -> Self {
        let mut daemon = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            // Socket abstraite : un daemon tué ne laisse aucun fichier (`/tmp/dbus-…`).
            .arg(format!(
                "--address=unix:abstract=/ultraclipboard-test/{}-{}",
                std::process::id(),
                NEXT_BUS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ))
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

    /// Ouvre des connexions à ce bus, comme `start_app` les demande.
    pub fn connector(
        &self,
    ) -> impl Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = zbus::Result<Connection>>>>
    {
        let address = self.address.clone();
        move || {
            let address = address.clone();
            Box::pin(async move { connection::Builder::address(address.as_str()).build().await })
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

    /// Signale une copie sans texte (image, fichiers…).
    pub async fn copied_non_text(&self) {
        self.conn
            .call_method(
                Some(BUS_NAME),
                OBJECT_PATH,
                Some(INTERFACE),
                "CopiedNonText",
                &(),
            )
            .await
            .unwrap();
    }

    /// Demande la restauration de l'Original (raccourci clavier).
    pub async fn restore_original(&self) {
        self.conn
            .call_method(
                Some(BUS_NAME),
                OBJECT_PATH,
                Some(INTERFACE),
                "RestoreOriginal",
                &(),
            )
            .await
            .unwrap();
    }

    /// Réapplique les Règles au texte du presse-papier (raccourci clavier).
    pub async fn reapply(&self, text: &str) {
        self.conn
            .call_method(
                Some(BUS_NAME),
                OBJECT_PATH,
                Some(INTERFACE),
                "Reapply",
                &(text,),
            )
            .await
            .unwrap();
    }

    /// Met en pause ou reprend (menu de l'icône).
    pub async fn set_paused(&self, paused: bool) {
        self.conn
            .call_method(
                Some(BUS_NAME),
                OBJECT_PATH,
                Some(INTERFACE),
                "SetPaused",
                &(paused,),
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

/// Notification reçue par le faux service de notifications du bureau.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub app_name: String,
    pub replaces_id: u32,
    pub summary: String,
    pub body: String,
}

/// Faux service de notifications du bureau (`org.freedesktop.Notifications`)
/// sur le bus privé : retient ce qu'on lui demande de montrer.
pub struct Desktop {
    _conn: Connection,
    shown: tokio::sync::mpsc::UnboundedReceiver<Shown>,
}

struct NotificationServer {
    shown: tokio::sync::mpsc::UnboundedSender<Shown>,
    next_id: std::sync::atomic::AtomicU32,
    /// Ne répond jamais au premier appel (service bloqué).
    silent_first: std::sync::atomic::AtomicBool,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    #[allow(clippy::too_many_arguments)]
    // `&self` : une méthode `&mut self` bloquée garderait l'objet verrouillé.
    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        _app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        _hints: std::collections::HashMap<String, zbus::OwnedVariant>,
        _expire_timeout: i32,
    ) -> u32 {
        if self
            .silent_first
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            std::future::pending::<()>().await;
        }
        let _ = self.shown.send(Shown {
            app_name,
            replaces_id,
            summary,
            body,
        });
        self.next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1
    }
}

impl Desktop {
    pub async fn start(bus: &PrivateBus) -> Self {
        Self::launch(bus, false).await
    }

    /// Service qui ne répond jamais à la première notification.
    pub async fn stuck_once(bus: &PrivateBus) -> Self {
        Self::launch(bus, true).await
    }

    async fn launch(bus: &PrivateBus, silent_first: bool) -> Self {
        let (tx, shown) = tokio::sync::mpsc::unbounded_channel();
        let conn = connection::Builder::address(bus.address.as_str())
            .name("org.freedesktop.Notifications")
            .serve_at(
                "/org/freedesktop/Notifications",
                NotificationServer {
                    shown: tx,
                    next_id: 0.into(),
                    silent_first: silent_first.into(),
                },
            )
            .build()
            .await
            .unwrap();
        Self { _conn: conn, shown }
    }

    /// Prochaine notification montrée, ou `None` s'il n'en vient aucune dans le délai.
    pub async fn next(&mut self, within: Duration) -> Option<Shown> {
        tokio::time::timeout(within, self.shown.recv())
            .await
            .ok()
            .flatten()
    }
}
