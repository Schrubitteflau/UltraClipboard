//! Moteur d'UltraClipboard : décide quoi écrire dans le presse-papier après une copie.
//!
//! Le moteur ne connaît ni D-Bus ni l'interface : l'appli lui signale les copies
//! et applique l'[`Outcome`] qu'il rend.
//!
//! Ce que fait le moteur est décrit par une [`Config`] : la première Règle dont le
//! motif correspond à la copie applique son Pipeline.

mod config;
mod pipeline;
mod state;
mod transformations;

use std::future::Future;
use std::sync::{Arc, Mutex};

pub use config::{Config, ConfigError, Notifications, Rule, SAMPLE_CONFIG};
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
    /// Aucune Règle ne correspond à la copie.
    NoRule,
    /// Le Pipeline rend le texte copié à l'identique.
    Unchanged,
    /// Le Pipeline rend un texte vide : on ne vide jamais le presse-papier.
    EmptyResult,
}

/// Réagit aux copies. Bon marché à cloner : les clones partagent le même état.
#[derive(Clone)]
pub struct Engine {
    config: Arc<Config>,
    state: Arc<Mutex<State>>,
}

impl Engine {
    /// Moteur qui suit cette config. `Config::default()` : aucune Règle, aucune
    /// copie transformée.
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            state: Arc::default(),
        }
    }

    /// Moteur qui applique ce Pipeline à toutes les copies.
    ///
    /// Seam de test temporaire : permet aux tests de brancher une étape lente.
    /// À retirer quand une vraie étape `command` (`sleep`) pourra jouer ce rôle.
    pub fn with_pipeline(pipeline: Pipeline) -> Self {
        Self::new(Config::match_all("test", pipeline))
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
        // Toute copie (même sans Règle) est enregistrée : elle annule celle en cours.
        let registration = self.state.lock().unwrap().register(text);
        let pipeline = self.config.rule_for(text).map(Rule::run);
        let state = Arc::clone(&self.state);
        let text = text.to_owned();
        async move {
            let ticket = match registration {
                Registration::OwnWrite => return Outcome::Nothing(Skip::OwnWrite),
                Registration::Accepted(ticket) => ticket,
            };
            let Some(pipeline) = pipeline else {
                return Outcome::Nothing(Skip::NoRule);
            };
            let result = pipeline.run(&text).await;
            let mut state = state.lock().unwrap();
            if !state.is_current(ticket) {
                return Outcome::Cancelled;
            }
            match result {
                Err(failure) => Outcome::Failed(failure),
                Ok(output) if output == text => Outcome::Nothing(Skip::Unchanged),
                Ok(output) if output.is_empty() => Outcome::Nothing(Skip::EmptyResult),
                Ok(output) => {
                    state.wrote(&output);
                    Outcome::Write(output)
                }
            }
        }
    }
}
