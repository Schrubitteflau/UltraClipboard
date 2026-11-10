//! Historique des transformations : en mémoire seulement, jamais sur disque.

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Serialize, Serializer};
use tokio::sync::watch;

use crate::state::Ticket;
use crate::{Outcome, Skip};

/// Nombre d'entrées gardées : les plus anciennes sortent.
pub const HISTORY_CAP: usize = 50;

/// Caractères gardés de chaque texte d'une entrée (Original, résultat, sortie
/// d'erreur) : au-delà, le texte est coupé et suivi de
/// « [… N caractères de plus] ». Borne la mémoire et ce qui part à la fenêtre.
pub const TEXT_CAP: usize = 65_536;

/// Une copie qui a déclenché un Pipeline, et ce qu'il en est sorti.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryEntry {
    /// Heure de la copie ; en millisecondes depuis l'époque Unix une fois sérialisée.
    #[serde(serialize_with = "millis")]
    pub at: SystemTime,
    /// Motif de la Règle choisie, tel qu'écrit.
    pub rule: String,
    /// Nom du Pipeline appliqué.
    pub pipeline: String,
    /// Texte copié.
    pub original: String,
    #[serde(flatten)]
    pub status: HistoryStatus,
}

/// Ce qu'a donné le Pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum HistoryStatus {
    /// Le Pipeline a abouti. Le presse-papier n'est écrit que si `result` diffère
    /// de l'Original.
    Success { result: String },
    /// Le Pipeline a échoué ; le presse-papier n'est pas touché. `step` est la
    /// Transformation en cause, `None` quand c'est le résultat du Pipeline entier
    /// qui est refusé (résultat vide).
    Failure {
        step: Option<String>,
        message: String,
        /// Sortie d'erreur de l'étape, vide si elle n'en a pas.
        stderr: String,
    },
    /// Une copie plus récente est arrivée avant la fin : résultat jeté.
    Cancelled,
}

/// Message d'un Pipeline refusé parce que son résultat est vide.
const EMPTY_RESULT: &str = "résultat vide";

impl HistoryStatus {
    /// Statut à retenir pour l'issue d'une copie qui a déclenché un Pipeline ;
    /// `None` quand aucun Pipeline n'a tourné (aucune Règle, écho).
    pub(crate) fn of(outcome: &Outcome, original: &str) -> Option<Self> {
        Some(match outcome {
            Outcome::Write(result) => Self::Success {
                result: result.clone(),
            },
            Outcome::Nothing(Skip::Unchanged) => Self::Success {
                result: original.to_owned(),
            },
            Outcome::Nothing(Skip::EmptyResult) => Self::Failure {
                step: None,
                message: EMPTY_RESULT.to_owned(),
                stderr: String::new(),
            },
            Outcome::Failed(failure) => Self::Failure {
                step: Some(failure.step.clone()),
                message: failure.message.clone(),
                stderr: failure.stderr.clone(),
            },
            Outcome::Cancelled => Self::Cancelled,
            Outcome::Nothing(Skip::NoRule | Skip::OwnWrite) => return None,
        })
    }
}

impl HistoryEntry {
    /// Coupe chaque texte à [`TEXT_CAP`] caractères.
    fn cap(&mut self) {
        cap(&mut self.original);
        match &mut self.status {
            HistoryStatus::Success { result } => cap(result),
            HistoryStatus::Failure { stderr, .. } => cap(stderr),
            HistoryStatus::Cancelled => {}
        }
    }
}

/// Garde les [`TEXT_CAP`] premiers caractères de `text`, puis le nombre de
/// caractères retirés.
fn cap(text: &mut String) {
    let Some((cut, _)) = text.char_indices().nth(TEXT_CAP) else {
        return;
    };
    let dropped = text[cut..].chars().count();
    text.truncate(cut);
    text.push_str(&format!("\n[… {dropped} caractères de plus]"));
}

fn millis<S: Serializer>(at: &SystemTime, serializer: S) -> Result<S::Ok, S::Error> {
    let ms = at
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    serializer.serialize_u64(u64::try_from(ms).unwrap_or(u64::MAX))
}

/// Entrées dans l'ordre des copies, la plus récente en tête.
pub(crate) struct History {
    entries: VecDeque<(Ticket, HistoryEntry)>,
    changes: watch::Sender<()>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            changes: watch::Sender::new(()),
        }
    }
}

impl History {
    /// Ajoute l'entrée d'une copie à sa place dans l'ordre des copies (une copie
    /// annulée peut finir après la suivante), puis retire les plus anciennes
    /// au-delà du plafond.
    pub(crate) fn record(&mut self, ticket: Ticket, mut entry: HistoryEntry) {
        entry.cap();
        let at = self
            .entries
            .iter()
            .position(|(other, _)| *other < ticket)
            .unwrap_or(self.entries.len());
        self.entries.insert(at, (ticket, entry));
        self.entries.truncate(HISTORY_CAP);
        self.changes.send_replace(());
    }

    pub(crate) fn entries(&self) -> Vec<HistoryEntry> {
        self.entries
            .iter()
            .map(|(_, entry)| entry.clone())
            .collect()
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<()> {
        self.changes.subscribe()
    }
}
