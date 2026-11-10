//! Appli UltraClipboard : expose le moteur sur le bus D-Bus de session.
//!
//! L'extension GNOME Shell appelle `Copied(text)` à chaque copie ; quand un
//! résultat est prêt, l'appli émet le signal `Apply(text)` et l'extension
//! l'écrit dans le presse-papier.

use engine::Engine;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::{connection, interface, object_server::SignalEmitter};

const BUS_NAME: &str = "org.ultraclipboard.UltraClipboard";
const OBJECT_PATH: &str = "/org/ultraclipboard/UltraClipboard";

struct Service {
    engine: Engine,
}

#[interface(name = "org.ultraclipboard.UltraClipboard1")]
impl Service {
    /// Signale une copie. Ne renvoie rien : le résultat arrive par `Apply`.
    async fn copied(&mut self, text: String, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        if let Some(output) = self.engine.on_copy(&text)
            && let Err(err) = Self::apply(&emitter, &output).await
        {
            eprintln!("ultraclipboard: échec d'émission de Apply : {err}");
        }
    }

    /// Demande à l'extension d'écrire ce texte dans le presse-papier.
    #[zbus(signal)]
    async fn apply(emitter: &SignalEmitter<'_>, text: &str) -> zbus::Result<()>;
}

#[tokio::main]
async fn main() -> zbus::Result<()> {
    let service = Service {
        engine: Engine::new(),
    };
    let conn = connection::Builder::session()
        .serve_at(OBJECT_PATH, service)
        .build()
        .await?;

    // Une seule instance : si le nom est déjà pris, on s'arrête au lieu d'attendre en file.
    match conn
        .request_name_with_flags(BUS_NAME, RequestNameFlags::DO_NOT_QUEUE)
        .await
    {
        Ok(RequestNameReply::PrimaryOwner) => {}
        Ok(_) | Err(zbus::Error::NameTaken) => {
            eprintln!("ultraclipboard: déjà lancé ({BUS_NAME} est pris)");
            std::process::exit(1);
        }
        Err(err) => return Err(err),
    }

    eprintln!("ultraclipboard: prêt sur {BUS_NAME}");
    tokio::signal::ctrl_c().await?;
    Ok(())
}
