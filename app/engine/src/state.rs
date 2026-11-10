//! État partagé du moteur entre les copies.

use tokio::sync::watch;

/// Numéro d'ordre d'une copie acceptée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Ticket(u64);

pub(crate) enum Registration {
    /// Écho de notre propre écriture : à ignorer.
    OwnWrite,
    /// Copie à traiter, et de quoi savoir quand une plus récente arrive.
    Accepted(Ticket, Superseded),
}

#[derive(Debug)]
pub(crate) struct State {
    /// Dernier texte écrit par UltraClipboard, retenu jusqu'à ce que son écho
    /// revienne ou qu'une écriture plus récente le remplace : sa copie en retour
    /// ne doit pas être retraitée.
    last_output: Option<String>,
    /// Numéro de la dernière copie acceptée, diffusé aux copies en cours.
    latest: watch::Sender<u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            last_output: None,
            latest: watch::Sender::new(0),
        }
    }
}

impl State {
    /// Enregistre une copie, dans l'ordre d'arrivée.
    ///
    /// L'écho de notre écriture est ignoré une fois et ne compte pas comme une
    /// nouvelle copie : il n'annule pas une copie en cours. Une copie sans rapport
    /// n'efface pas l'écriture retenue, dont l'écho peut encore arriver après elle.
    pub(crate) fn register(&mut self, text: &str) -> Registration {
        if self.last_output.as_deref() == Some(text) {
            self.last_output = None;
            return Registration::OwnWrite;
        }
        let ticket = self.supersede();
        Registration::Accepted(
            ticket,
            Superseded {
                ticket,
                latest: self.latest.subscribe(),
            },
        )
    }

    /// Enregistre une copie sans texte : rien à traiter, mais elle rend la
    /// copie en cours dépassée.
    pub(crate) fn register_without_text(&mut self) {
        self.supersede();
    }

    /// Numéro de la nouvelle dernière copie ; les copies en cours sont dépassées.
    fn supersede(&mut self) -> Ticket {
        self.latest.send_modify(|latest| *latest += 1);
        Ticket(*self.latest.borrow())
    }

    /// Vrai si aucune copie n'a été acceptée depuis celle-ci.
    pub(crate) fn is_current(&self, ticket: Ticket) -> bool {
        ticket.0 == *self.latest.borrow()
    }

    /// Retient ce qu'on écrit, pour reconnaître son écho.
    pub(crate) fn wrote(&mut self, text: &str) {
        self.last_output = Some(text.to_owned());
    }
}

/// Se termine dès qu'une copie plus récente est acceptée.
#[derive(Debug)]
pub(crate) struct Superseded {
    ticket: Ticket,
    latest: watch::Receiver<u64>,
}

impl Superseded {
    pub(crate) async fn wait(mut self) {
        // L'émetteur vit dans l'état partagé, que le traitement en cours garde :
        // il ne disparaît pas tant qu'on attend.
        let _ = self
            .latest
            .wait_for(|&latest| latest != self.ticket.0)
            .await;
    }
}
