//! Service D-Bus d'UltraClipboard, sans fenêtre : utilisé par l'hôte Tauri
//! (`app/desktop`, binaire `ultraclipboard`) et par les tests de contrat.
//!
//! L'extension GNOME Shell appelle `Copied(text)` à chaque copie de texte, et
//! `CopiedNonText()` à chaque copie sans texte (image, fichiers…) ; quand un
//! résultat est prêt, le service émet le signal `Apply(text)` et l'extension
//! l'écrit dans le presse-papier. `RestoreOriginal()` (raccourci de
//! l'extension) annule la dernière Transformation par un `Apply` de
//! l'Original. `Reapply(text)` (raccourci de l'extension) réapplique les
//! Règles au presse-papier actuel, même en pause ; `SetPaused(paused)` met en
//! pause ou reprend. Toute copie, texte ou non, arrête le traitement de la
//! précédente.

mod app;
mod config;
mod notifications;

use std::fmt;

use engine::{Engine, Outcome};
use tokio::sync::mpsc::UnboundedReceiver;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::{Connection, interface, object_server::SignalEmitter};

pub use app::{Started, start_app};
pub use config::{ConfigFollower, config_path, follow_config, start_engine};
pub use notifications::{NotificationSender, PendingNotifications, queue_notifications};

pub const BUS_NAME: &str = "org.ultraclipboard.UltraClipboard";
pub const OBJECT_PATH: &str = "/org/ultraclipboard/UltraClipboard";
pub const INTERFACE: &str = "org.ultraclipboard.UltraClipboard1";

/// Pourquoi le service n'a pas pu démarrer.
#[derive(Debug)]
pub enum ServeError {
    /// Une autre instance tient déjà le nom sur le bus.
    AlreadyRunning,
    Bus(zbus::Error),
}

impl fmt::Display for ServeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning => write!(f, "déjà lancé ({BUS_NAME} est pris)"),
            Self::Bus(err) => write!(f, "bus D-Bus : {err}"),
        }
    }
}

impl std::error::Error for ServeError {}

impl From<zbus::Error> for ServeError {
    fn from(err: zbus::Error) -> Self {
        Self::Bus(err)
    }
}

/// Expose le moteur sur `conn` et prend le nom du service.
///
/// Rend la main une fois le service en place ; il tourne jusqu'à la fin du
/// processus (ou de son runtime tokio), même si l'appelant lâche `conn` :
/// l'émetteur de `Apply` en garde une copie, et la file qu'il vide reste
/// ouverte tant que le moteur vit, lui-même tenu par le service de `conn`.
/// Une seule instance : si le nom est déjà pris, renvoie [`ServeError::AlreadyRunning`]
/// au lieu d'attendre en file.
pub async fn serve(conn: &Connection, engine: Engine) -> Result<(), ServeError> {
    // Abonné avant toute copie : aucune écriture décidée ne se perd.
    let writes = engine.writes();
    conn.object_server()
        .at(OBJECT_PATH, Service { engine })
        .await?;
    match conn
        .request_name_with_flags(BUS_NAME, RequestNameFlags::DO_NOT_QUEUE)
        .await
    {
        Ok(RequestNameReply::PrimaryOwner) => {
            emit_writes(SignalEmitter::new(conn, OBJECT_PATH)?.into_owned(), writes);
            Ok(())
        }
        Ok(_) | Err(zbus::Error::NameTaken) => Err(ServeError::AlreadyRunning),
        Err(err) => Err(err.into()),
    }
}

/// Seul émetteur de `Apply` : un à un, dans l'ordre où le moteur décide les
/// écritures (résultats et restaurations). Sans lui, chaque copie émettrait le
/// sien depuis sa tâche, et celui d'une copie dépassée pourrait arriver après
/// celui d'une plus récente : la dernière copie ne gagnerait plus (US12).
/// Aucune n'est sautée : l'anti-boucle attend l'écho de chacune.
fn emit_writes(emitter: SignalEmitter<'static>, mut writes: UnboundedReceiver<String>) {
    tokio::spawn(async move {
        while let Some(text) = writes.recv().await {
            if let Err(err) = Service::apply(&emitter, &text).await {
                eprintln!("ultraclipboard: échec d'émission de Apply : {err}");
            }
        }
    });
}

struct Service {
    engine: Engine,
}

// `spawn = false` : zbus traite les appels un à un, dans l'ordre d'arrivée sur le
// bus, au lieu d'une tâche par appel qui pourrait réordonner une rafale de copies.
// Sans risque de blocage : `copied` ne fait que lancer le traitement en tâche de fond.
#[interface(name = "org.ultraclipboard.UltraClipboard1", spawn = false)]
impl Service {
    /// Signale une copie. Rend la main aussitôt : le traitement part en tâche de
    /// fond et son résultat arrive par `Apply`.
    fn copied(&self, text: String) {
        Self::settle(self.engine.on_copy(&text));
    }

    /// Réapplique les Règles à `text`, le contenu actuel du presse-papier
    /// (raccourci de l'extension) : comme `Copied`, mais aussi en pause et même
    /// si `text` est notre propre écriture. Résultat par `Apply`.
    fn reapply(&self, text: String) {
        Self::settle(self.engine.reapply(&text));
    }

    /// Met en pause (aucune copie transformée) ou reprend, jusqu'au redémarrage.
    fn set_paused(&self, paused: bool) {
        self.engine.set_paused(paused);
    }

    /// Signale une copie sans texte (image, fichiers…) : rien à transformer,
    /// mais le traitement d'une copie précédente s'arrête et son résultat ne
    /// remplace jamais ce qui vient d'être copié.
    fn copied_non_text(&self) {
        self.engine.on_copy_without_text();
    }

    /// Restaure l'Original (raccourci de l'extension) : `Apply` avec le texte
    /// d'avant la dernière Transformation appliquée. Sans Original, ou si une
    /// copie a été faite depuis : rien n'est écrit, et la copie en cours de
    /// traitement, s'il y en a une, s'arrête et reste telle que copiée.
    fn restore_original(&self) {
        // L'Original, s'il y en a un, part par `Apply` depuis `emit_writes`.
        self.engine.restore_original();
    }

    /// Demande à l'extension d'écrire ce texte dans le presse-papier.
    #[zbus(signal)]
    async fn apply(emitter: &SignalEmitter<'_>, text: &str) -> zbus::Result<()>;
}

impl Service {
    /// Lance le traitement en tâche de fond ; un texte à écrire part par
    /// `Apply` depuis `emit_writes`, publié par le moteur au moment décidé.
    fn settle(work: impl std::future::Future<Output = Outcome> + Send + 'static) {
        tokio::spawn(async move {
            match work.await {
                // Jamais la sortie d'erreur : elle peut contenir le texte copié et
                // ce journal est persistant. Elle reste dans l'historique, en mémoire.
                Outcome::Failed(failure) => eprintln!(
                    "ultraclipboard: échec de l'étape {} : {}",
                    failure.step, failure.message
                ),
                Outcome::Write(_) | Outcome::Nothing(_) | Outcome::Cancelled => {}
            }
        });
    }
}
