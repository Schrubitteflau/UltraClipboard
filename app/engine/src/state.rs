//! État partagé du moteur entre les copies.

/// Numéro d'ordre d'une copie acceptée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ticket(u64);

pub(crate) enum Registration {
    /// Écho de notre propre écriture : à ignorer.
    OwnWrite,
    /// Copie à traiter.
    Accepted(Ticket),
}

#[derive(Debug, Default)]
pub(crate) struct State {
    /// Dernier texte écrit par UltraClipboard, retenu jusqu'à ce que son écho
    /// revienne ou qu'une écriture plus récente le remplace : sa copie en retour
    /// ne doit pas être retraitée.
    last_output: Option<String>,
    /// Numéro de la dernière copie acceptée.
    latest: u64,
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
        self.latest += 1;
        Registration::Accepted(Ticket(self.latest))
    }

    /// Vrai si aucune copie n'a été acceptée depuis celle-ci.
    pub(crate) fn is_current(&self, ticket: Ticket) -> bool {
        ticket.0 == self.latest
    }

    /// Retient ce qu'on écrit, pour reconnaître son écho.
    pub(crate) fn wrote(&mut self, text: &str) {
        self.last_output = Some(text.to_owned());
    }
}
