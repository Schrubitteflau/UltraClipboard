//! Envoi des notifications décidées par le moteur au service de notifications
//! standard du bureau (`org.freedesktop.Notifications`, bus de session).

use std::collections::HashMap;
use std::time::Duration;

use engine::{Engine, Notification};
use tokio::sync::broadcast::{self, error::RecvError};
use zbus::{Connection, Variant};

const APP_NAME: &str = "UltraClipboard";

/// Délai de réponse du service de notifications : au-delà, la notification est
/// tenue pour perdue et la suivante part.
const NOTIFY_TIMEOUT: Duration = Duration::from_secs(3);

/// Longueur maximale (en caractères) d'une ligne de notification.
const MAX_LINE: usize = 300;

/// Notifications retenues depuis l'abonnement, en attente d'une connexion.
pub struct PendingNotifications {
    notices: broadcast::Receiver<Notification>,
}

/// Retient, dès maintenant, les notifications que `engine` décide d'envoyer.
/// À appeler avant de charger la config, pour qu'une config invalide au lancement
/// soit notifiée une fois le service en place ([`PendingNotifications::send_on`]).
/// Lâché sans être envoyé (une seconde instance refusée), rien n'est montré.
pub fn queue_notifications(engine: &Engine) -> PendingNotifications {
    PendingNotifications {
        notices: engine.subscribe_notifications(),
    }
}

/// Envoie les notifications tant qu'il vit ; s'arrête quand on le lâche.
pub struct NotificationSender {
    task: tokio::task::JoinHandle<()>,
}

impl Drop for NotificationSender {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl PendingNotifications {
    /// Montre sur le bureau, par `conn`, les notifications retenues puis les
    /// suivantes.
    ///
    /// `conn` doit être une connexion à part, pas celle du service : un message
    /// que le bus refuserait ne doit jamais couper le service. Service de
    /// notifications absent, en erreur ou muet : la notification est perdue
    /// (message sur la sortie d'erreur), l'appli continue.
    ///
    /// Un nouveau succès remplace la bulle du succès précédent, une nouvelle
    /// config invalide celle de la précédente ; un échec n'est jamais remplacé.
    ///
    /// À appeler dans un runtime tokio : l'envoi tourne en tâche de fond.
    pub fn send_on(self, conn: Connection) -> NotificationSender {
        let mut notices = self.notices;
        let task = tokio::spawn(async move {
            let mut last_success = 0;
            let mut last_config = 0;
            loop {
                let notice = match notices.recv().await {
                    Ok(notice) => notice,
                    Err(RecvError::Lagged(lost)) => {
                        eprintln!(
                            "ultraclipboard: {lost} notification(s) perdue(s) (trop d'un coup)"
                        );
                        continue;
                    }
                    Err(RecvError::Closed) => return,
                };
                let mut never = 0;
                let replaced = match notice {
                    Notification::Succeeded { .. } => &mut last_success,
                    Notification::ConfigRejected(_) => &mut last_config,
                    _ => &mut never,
                };
                let (summary, body) = text(&notice);
                match show(&conn, *replaced, &summary, &body).await {
                    Ok(id) => *replaced = id,
                    Err(err) => eprintln!("ultraclipboard: notification impossible : {err}"),
                }
            }
        });
        NotificationSender { task }
    }
}

/// Titre et corps de la notification.
///
/// Échec : l'étape et son message, puis sa cause probable (dernière ligne de la
/// sortie d'erreur).
fn text(notice: &Notification) -> (String, String) {
    let (summary, body) = match notice {
        Notification::Succeeded { pipeline } => (format!("✓ {pipeline}"), String::new()),
        Notification::Failed { pipeline, failure } => {
            let mut body = format!("{} : {}", failure.step, failure.message);
            if let Some(cause) = failure.cause() {
                body = format!("{body}\n{cause}");
            }
            (format!("✗ {pipeline}"), body)
        }
        Notification::EmptyResult { pipeline } => (
            format!("✗ {pipeline}"),
            "résultat vide : presse-papier inchangé".to_owned(),
        ),
        Notification::ConfigRejected(err) => ("Config invalide".to_owned(), err.to_string()),
    };
    (clean(&summary), escape(&clean(&body)))
}

/// Texte acceptable par le bus et lisible dans une bulle : sans octet nul (que
/// le bus refuse), lignes coupées à [`MAX_LINE`] caractères.
fn clean(text: &str) -> String {
    text.replace('\0', "")
        .split('\n')
        .map(|line| match line.char_indices().nth(MAX_LINE) {
            Some((at, _)) => format!("{}…", &line[..at]),
            None => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Échappe le corps, que GNOME Shell (seule cible, D4) lit comme du balisage ;
/// le titre est du texte brut.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Montre la notification (en remplaçant la bulle `replaces`, si non nul) et
/// rend son numéro.
async fn show(conn: &Connection, replaces: u32, summary: &str, body: &str) -> zbus::Result<u32> {
    let actions: &[&str] = &[];
    let hints: HashMap<&str, Variant<'_>> = HashMap::new();
    let args = (APP_NAME, replaces, "", summary, body, actions, hints, -1i32);
    let call = conn.call_method(
        Some("org.freedesktop.Notifications"),
        "/org/freedesktop/Notifications",
        Some("org.freedesktop.Notifications"),
        "Notify",
        &args,
    );
    let reply = tokio::time::timeout(NOTIFY_TIMEOUT, call)
        .await
        .map_err(|_| {
            zbus::Error::Failure(format!(
                "pas de réponse du service en {} s",
                NOTIFY_TIMEOUT.as_secs()
            ))
        })??;
    reply.body().deserialize()
}
