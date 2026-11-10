//! Service D-Bus d'UltraClipboard, sans fenêtre : utilisé par le binaire, par les
//! tests de contrat et, plus tard, par l'hôte Tauri.
//!
//! L'extension GNOME Shell appelle `Copied(text)` à chaque copie ; quand un
//! résultat est prêt, le service émet le signal `Apply(text)` et l'extension
//! l'écrit dans le presse-papier.

use std::fmt;

use engine::{Engine, Outcome};
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::{Connection, interface, object_server::SignalEmitter};

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
/// Rend la main une fois le service en place ; il tourne tant que `conn` vit.
/// Une seule instance : si le nom est déjà pris, renvoie [`ServeError::AlreadyRunning`]
/// au lieu d'attendre en file.
pub async fn serve(conn: &Connection, engine: Engine) -> Result<(), ServeError> {
    conn.object_server()
        .at(OBJECT_PATH, Service { engine })
        .await?;
    match conn
        .request_name_with_flags(BUS_NAME, RequestNameFlags::DO_NOT_QUEUE)
        .await
    {
        Ok(RequestNameReply::PrimaryOwner) => Ok(()),
        Ok(_) | Err(zbus::Error::NameTaken) => Err(ServeError::AlreadyRunning),
        Err(err) => Err(err.into()),
    }
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
    fn copied(&self, text: String, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let work = self.engine.on_copy(&text);
        let emitter = emitter.into_owned();
        tokio::spawn(async move {
            match work.await {
                Outcome::Write(output) => {
                    if let Err(err) = Self::apply(&emitter, &output).await {
                        eprintln!("ultraclipboard: échec d'émission de Apply : {err}");
                    }
                }
                Outcome::Failed(failure) => eprintln!(
                    "ultraclipboard: échec de l'étape {} : {}\n{}",
                    failure.step, failure.message, failure.stderr
                ),
                Outcome::Nothing(_) | Outcome::Cancelled => {}
            }
        });
    }

    /// Demande à l'extension d'écrire ce texte dans le presse-papier.
    #[zbus(signal)]
    async fn apply(emitter: &SignalEmitter<'_>, text: &str) -> zbus::Result<()>;
}
