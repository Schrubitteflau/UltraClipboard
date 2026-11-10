//! État partagé du moteur entre les copies.
//!
//! Deux compteurs : le numéro du dernier traitement lancé (copie ou
//! réapplication), qui arrête celui d'avant, et celui de la dernière vraie
//! copie, qui seule change le presse-papier : les échos attendus et l'Original
//! restaurable ne survivent pas à une copie, mais survivent à une réapplication,
//! à une restauration ou à une mise en pause.

use std::collections::VecDeque;

use tokio::sync::watch;

/// Nombre d'écritures dont on attend encore l'écho, au plus : de quoi couvrir
/// un résultat et deux restaurations faites avant son retour. Au-delà, la plus
/// ancienne est oubliée.
const PENDING_ECHOES: usize = 4;

/// Numéro d'ordre d'un traitement (copie acceptée ou réapplication).
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
    /// Textes écrits par UltraClipboard dont l'écho n'est pas encore revenu,
    /// du plus ancien au plus récent ([`PENDING_ECHOES`] au plus), avec le
    /// numéro de la dernière copie au moment de l'écriture : leur copie en
    /// retour ne doit pas être retraitée. Chacun est ignoré une fois.
    pending_echoes: VecDeque<(Ticket, String)>,
    /// Original de la dernière Transformation appliquée, avec le numéro du
    /// traitement (copie ou réapplication) qui l'a appliquée.
    original: Option<(Ticket, String)>,
    /// Numéro du dernier traitement lancé, diffusé aux traitements en cours :
    /// tout autre traitement s'arrête.
    latest: watch::Sender<u64>,
    /// Numéro de la dernière vraie copie (avec ou sans texte).
    last_copy: Ticket,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pending_echoes: VecDeque::new(),
            original: None,
            latest: watch::Sender::new(0),
            last_copy: Ticket(0),
        }
    }
}

impl State {
    /// Enregistre une copie, dans l'ordre d'arrivée.
    ///
    /// L'écho de notre écriture est ignoré une fois et ne compte pas comme une
    /// nouvelle copie : il n'annule pas une copie en cours. Une copie sans rapport
    /// n'efface pas les écritures retenues, dont l'écho peut encore arriver après
    /// elle ; une écriture plus récente faite sans nouvelle copie non plus
    /// (restauration ou réapplication faite avant le retour de l'écho du
    /// résultat). Si les deux lectures de l'extension voient le texte restauré
    /// (deux échos identiques), le second est traité comme une copie : rare, et
    /// on ne peut pas le distinguer d'une vraie copie.
    pub(crate) fn register(&mut self, text: &str) -> Registration {
        if let Some(echo) = self.pending_echoes.iter().position(|(_, w)| w == text) {
            self.pending_echoes.remove(echo);
            return Registration::OwnWrite;
        }
        self.last_copy = self.supersede();
        self.accept(self.last_copy)
    }

    /// Enregistre une réapplication : traitée comme une copie de ce texte, même
    /// s'il est notre propre écriture, et elle arrête le traitement en cours.
    /// Mais ce n'est pas une copie (le presse-papier n'a pas changé) : les
    /// échos attendus restent attendus et l'Original reste restaurable tant
    /// qu'elle n'applique pas de Transformation.
    pub(crate) fn register_explicit(&mut self) -> Registration {
        let ticket = self.supersede();
        self.accept(ticket)
    }

    fn accept(&self, ticket: Ticket) -> Registration {
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
        self.last_copy = self.supersede();
    }

    /// Arrête le traitement en cours, s'il y en a un (mise en pause), sans
    /// rien changer d'autre.
    pub(crate) fn cancel(&mut self) {
        self.supersede();
    }

    /// Numéro du nouveau dernier traitement ; ceux en cours sont dépassés.
    fn supersede(&mut self) -> Ticket {
        self.latest.send_modify(|latest| *latest += 1);
        Ticket(*self.latest.borrow())
    }

    /// Vrai si aucun traitement n'a été lancé depuis celui-ci.
    pub(crate) fn is_current(&self, ticket: Ticket) -> bool {
        ticket.0 == *self.latest.borrow()
    }

    /// Le traitement `ticket` (texte `original`) est transformé en `output`,
    /// qu'on écrit : retient son écho à ignorer et l'Original à restaurer.
    pub(crate) fn transformed(&mut self, ticket: Ticket, original: &str, output: &str) {
        self.wrote(output);
        self.original = Some((ticket, original.to_owned()));
    }

    /// Texte à écrire pour annuler la dernière Transformation appliquée, retenu
    /// comme notre écriture (son écho est ignoré).
    ///
    /// Le traitement en cours, s'il y en a un, s'arrête. La dernière copie
    /// gagne : si une copie a été faite depuis cette Transformation (ou sans
    /// Original), rien n'est écrit et cette copie reste telle que copiée.
    pub(crate) fn restore(&mut self) -> Option<String> {
        self.supersede();
        let original = match &self.original {
            Some((ticket, original)) if *ticket >= self.last_copy => original.clone(),
            _ => return None,
        };
        self.wrote(&original);
        Some(original)
    }

    /// Retient une écriture, pour reconnaître son écho.
    ///
    /// Les écritures faites avant une copie plus récente sont oubliées : leur
    /// écho, s'il n'est pas revenu jusqu'ici, est perdu (l'extension a lu la
    /// copie suivante à sa place), et une vraie copie du même texte ne doit pas
    /// être ignorée plus tard.
    fn wrote(&mut self, text: &str) {
        let last_copy = self.last_copy;
        self.pending_echoes.retain(|(at, _)| *at == last_copy);
        if self.pending_echoes.len() == PENDING_ECHOES {
            self.pending_echoes.pop_front();
        }
        self.pending_echoes.push_back((last_copy, text.to_owned()));
    }
}

/// Se termine dès qu'un traitement plus récent est lancé.
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
