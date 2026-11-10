//! Démarrage de l'appli, dans l'ordre que suit le binaire.

use std::future::Future;
use std::path::PathBuf;

use engine::Engine;
use zbus::Connection;

use crate::config::follow_config_at;
use crate::{ConfigFollower, NotificationSender, ServeError, queue_notifications, serve};

/// Appli démarrée : le service tourne jusqu'à la fin du processus, même si on
/// lâche `conn` (voir [`serve`]) ; l'envoi des notifications et le suivi de la
/// config s'arrêtent quand on la lâche.
pub struct Started {
    pub conn: Connection,
    pub engine: Engine,
    _sender: Option<NotificationSender>,
    _follower: Option<ConfigFollower>,
}

/// Démarre l'appli : charge la config `path` (puis la suit à chaque
/// enregistrement), expose le moteur sur le bus, puis envoie ses notifications
/// sur le bureau. `connect` ouvre une connexion au bus de session ; elle est
/// appelée deux fois : le service et, à part, les notifications (un message
/// refusé par le bus ne coupe jamais le service).
///
/// Les notifications sont retenues avant le premier chargement (une config
/// invalide au lancement est notifiée) mais envoyées seulement une fois le nom
/// pris : une seconde instance refusée ne montre rien.
///
/// Sans `path` (ni XDG_CONFIG_HOME ni HOME), aucune Règle n'est active. Le
/// moteur est celui de [`crate::start_engine`], la file de notifications en plus.
pub async fn start_app<F, Fut>(path: Option<PathBuf>, connect: F) -> Result<Started, ServeError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = zbus::Result<Connection>>,
{
    // Aucune config chargée tant que le fichier n'est pas lu.
    let engine = Engine::default();
    let pending = queue_notifications(&engine);
    let follower = follow_config_at(path, &engine);
    let conn = connect().await?;
    serve(&conn, engine.clone()).await?;
    let sender = match connect().await {
        Ok(notifications) => Some(pending.send_on(notifications)),
        Err(err) => {
            eprintln!("ultraclipboard: pas de notifications, bus de session : {err}");
            None
        }
    };
    Ok(Started {
        conn,
        engine,
        _sender: sender,
        _follower: follower,
    })
}
