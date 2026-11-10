//! Notifications : ce que le moteur juge bon de signaler à l'utilisateur.
//!
//! Le moteur décide ; l'appli ne fait qu'envoyer ce qu'elle reçoit.

use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::broadcast;

use crate::config::{ConfigError, Notifications};
use crate::pipeline::Failure;

/// Évènement à montrer à l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notification {
    /// Un Pipeline a fini et son résultat est dans le presse-papier.
    Succeeded { pipeline: String },
    /// Une étape du Pipeline a échoué : le presse-papier n'a pas été touché.
    Failed { pipeline: String, failure: Failure },
    /// Le Pipeline a rendu un texte vide : le presse-papier n'a pas été touché
    /// (on ne le vide jamais). Compte comme un échec.
    EmptyResult { pipeline: String },
    /// La config a été refusée au chargement ou au rechargement (chaque
    /// enregistrement d'un fichier invalide) : la config active reste en place.
    ConfigRejected(ConfigError),
}

/// Notifications en attente d'un abonné ; au-delà, les plus anciennes sont perdues.
const BACKLOG: usize = 32;

impl Notification {
    /// Vrai si le niveau `level` de la config demande de la montrer.
    fn wanted(&self, level: Notifications) -> bool {
        match (level, self) {
            (Notifications::All, _) => true,
            (Notifications::Errors, Notification::Succeeded { .. }) => false,
            (Notifications::Errors, _) => true,
            (Notifications::Off, _) => false,
        }
    }
}

/// Décide des notifications et diffuse celles à montrer.
pub(crate) struct Notifier {
    sender: broadcast::Sender<Notification>,
    /// Coupées pour la session (menu de l'icône), jusqu'au redémarrage.
    muted: AtomicBool,
}

impl Default for Notifier {
    fn default() -> Self {
        Self {
            sender: broadcast::Sender::new(BACKLOG),
            muted: AtomicBool::new(false),
        }
    }
}

impl Notifier {
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<Notification> {
        self.sender.subscribe()
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    pub(crate) fn muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// Diffuse `notification` si la session n'est pas coupée et que le niveau
    /// `level` de la config active la demande.
    pub(crate) fn send(&self, level: Notifications, notification: Notification) {
        if self.muted() || !notification.wanted(level) {
            return;
        }
        // Sans abonné, la notification est perdue : personne pour la montrer.
        let _ = self.sender.send(notification);
    }
}
