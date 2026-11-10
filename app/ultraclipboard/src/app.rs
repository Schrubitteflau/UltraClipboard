//! Démarrage de l'appli, dans l'ordre que suit le binaire.

use std::future::Future;
use std::path::PathBuf;

use engine::{Config, Engine};
use zbus::Connection;

use crate::{
    ConfigFollower, NotificationSender, ServeError, follow_config, queue_notifications, serve,
};

/// Appli démarrée : le service tourne tant que `conn` vit ; l'envoi des
/// notifications et le suivi de la config s'arrêtent quand on la lâche.
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
/// Sans `path` (ni XDG_CONFIG_HOME ni HOME), aucune Règle n'est active.
pub async fn start_app<F, Fut>(path: Option<PathBuf>, connect: F) -> Result<Started, ServeError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = zbus::Result<Connection>>,
{
    let engine = Engine::new(Config::default());
    let pending = queue_notifications(&engine);
    let follower = match path {
        Some(path) => Some(follow_config(path, engine.clone())),
        None => {
            eprintln!("ultraclipboard: ni XDG_CONFIG_HOME ni HOME, aucune Règle active");
            None
        }
    };
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
