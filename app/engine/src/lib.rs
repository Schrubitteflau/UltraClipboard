//! Moteur d'UltraClipboard : décide quoi écrire dans le presse-papier après une copie.
//!
//! Le moteur ne connaît ni D-Bus ni l'interface : l'appli lui signale les copies
//! et applique l'[`Outcome`] qu'il rend.
//!
//! Étape actuelle : un seul Pipeline codé en dur, `upper` (remplacé par les Règles
//! de la config au ticket #3).

mod config;
mod pipeline;
mod state;
mod transformations;

use std::future::Future;
use std::sync::{Arc, Mutex};

pub use pipeline::{Failure, Pipeline};
pub use transformations::{StepError, StepFuture, Transformation};

use state::{Registration, State};

/// Issue d'une copie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Texte à écrire dans le presse-papier.
    Write(String),
    /// Rien à faire : le presse-papier reste tel quel.
    Nothing(Skip),
    /// Une étape du Pipeline a échoué ; le presse-papier reste tel quel.
    Failed(Failure),
    /// Une copie plus récente est arrivée pendant le traitement ; le résultat est jeté.
    Cancelled,
}

/// Pourquoi une copie ne donne rien à écrire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// La copie est l'écho de ce qu'UltraClipboard vient d'écrire (anti-boucle).
    OwnWrite,
    /// Le Pipeline rend le texte copié à l'identique.
    Unchanged,
}

/// Réagit aux copies. Bon marché à cloner : les clones partagent le même état.
#[derive(Clone)]
pub struct Engine {
    pipeline: Arc<Pipeline>,
    state: Arc<Mutex<State>>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Moteur avec le Pipeline par défaut (`upper`).
    pub fn new() -> Self {
        Self::with_pipeline(Pipeline::new(vec![Box::new(transformations::Upper)]))
    }

    /// Moteur qui applique ce Pipeline à toutes les copies.
    ///
    /// Seam de test temporaire : permet aux tests de brancher une étape lente.
    /// À retirer quand une vraie étape `command` (`sleep`) pourra jouer ce rôle.
    pub fn with_pipeline(pipeline: Pipeline) -> Self {
        Self {
            pipeline: Arc::new(pipeline),
            state: Arc::default(),
        }
    }

    /// Signale une copie.
    ///
    /// La copie est enregistrée dès l'appel : l'ordre des appels est l'ordre des
    /// copies, donc l'appelant doit appeler `on_copy` dans l'ordre d'arrivée (pour
    /// le service D-Bus, l'ordre des appels `Copied` sur le bus). Le traitement se
    /// fait dans le futur rendu, qui ne dépend pas de `self` et peut être lancé en
    /// tâche de fond. Si une autre copie est signalée avant qu'il se termine, son
    /// issue est [`Outcome::Cancelled`] et son résultat est jeté.
    pub fn on_copy(&self, text: &str) -> impl Future<Output = Outcome> + Send + use<> {
        let registration = self.state.lock().unwrap().register(text);
        let pipeline = Arc::clone(&self.pipeline);
        let state = Arc::clone(&self.state);
        let text = text.to_owned();
        async move {
            let ticket = match registration {
                Registration::OwnWrite => return Outcome::Nothing(Skip::OwnWrite),
                Registration::Accepted(ticket) => ticket,
            };
            let result = pipeline.run(&text).await;
            let mut state = state.lock().unwrap();
            if !state.is_current(ticket) {
                return Outcome::Cancelled;
            }
            match result {
                Err(failure) => Outcome::Failed(failure),
                Ok(output) if output == text => Outcome::Nothing(Skip::Unchanged),
                Ok(output) => {
                    state.wrote(&output);
                    Outcome::Write(output)
                }
            }
        }
    }
}
