//! Moteur d'UltraClipboard : décide quoi écrire dans le presse-papier après une copie.
//!
//! Le moteur ne connaît ni D-Bus ni l'interface : l'appli lui signale les copies
//! et applique l'[`Outcome`] qu'il rend.
//!
//! Ce que fait le moteur est décrit par une [`Config`] : la première Règle dont le
//! motif correspond à la copie applique son Pipeline. La config active peut être
//! remplacée à tout moment ([`Engine::load`]) sans perdre l'état du moteur.

mod config;
mod pipeline;
mod state;
mod transformations;

use std::future::Future;
use std::sync::{Arc, Mutex, RwLock};

pub use config::{Config, ConfigError, Notifications, Rule, SAMPLE_CONFIG};
pub use pipeline::Failure;
pub use transformations::Endpoints;

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
    config: Arc<RwLock<Loaded>>,
    state: Arc<Mutex<State>>,
}

/// Config active et refus du dernier chargement, s'il a échoué.
struct Loaded {
    active: Arc<Config>,
    error: Option<ConfigError>,
}

impl Engine {
    /// Moteur qui suit cette config. `Config::default()` : aucune Règle, aucune
    /// copie transformée.
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(RwLock::new(Loaded {
                active: Arc::new(config),
                error: None,
            })),
            state: Arc::default(),
        }
    }

    /// Charge une config YAML et la rend active pour les copies suivantes.
    ///
    /// Une copie déjà en cours finit avec le Pipeline qu'elle avait choisi. L'état
    /// du moteur (anti-boucle, copie en cours) n'est pas touché. Config refusée :
    /// l'active reste en place et l'erreur est retenue ([`Engine::config_error`]).
    pub fn load(&self, yaml: &str) -> Result<(), ConfigError> {
        let parsed = Config::parse(yaml);
        let mut loaded = self.config.write().unwrap();
        match parsed {
            Ok(config) => {
                loaded.active = Arc::new(config);
                loaded.error = None;
                Ok(())
            }
            Err(err) => {
                loaded.error = Some(err.clone());
                Err(err)
            }
        }
    }

    /// La config n'a pas pu être lue (fichier illisible…) : l'active reste en
    /// place et cette erreur est retenue comme celle d'un chargement refusé.
    pub fn reject_config(&self, error: ConfigError) {
        self.config.write().unwrap().error = Some(error);
    }

    /// Config active.
    pub fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config.read().unwrap().active)
    }

    /// Pourquoi le dernier chargement a été refusé ; `None` s'il a réussi (ou s'il
    /// n'y en a pas eu).
    pub fn config_error(&self) -> Option<ConfigError> {
        self.config.read().unwrap().error.clone()
    }

    /// Signale une copie sans texte (image, fichiers…).
    ///
    /// Rien n'est écrit, mais c'est une copie plus récente : le traitement
    /// d'une copie en cours s'arrête, son issue est [`Outcome::Cancelled`] et
    /// son résultat ne remplace jamais ce qui vient d'être copié.
    pub fn on_copy_without_text(&self) {
        self.state.lock().unwrap().register_without_text();
    }

    /// Signale une copie.
    ///
    /// La copie est enregistrée dès l'appel : l'ordre des appels est l'ordre des
    /// copies, donc l'appelant doit appeler `on_copy` dans l'ordre d'arrivée (pour
    /// le service D-Bus, l'ordre des appels `Copied` sur le bus). Le traitement se
    /// fait dans le futur rendu, qui ne dépend pas de `self` et peut être lancé en
    /// tâche de fond. Si une autre copie est signalée avant qu'il se termine, il
    /// s'arrête aussitôt (l'étape en cours est abandonnée : processus tué, requête
    /// coupée) et son issue est [`Outcome::Cancelled`].
    pub fn on_copy(&self, text: &str) -> impl Future<Output = Outcome> + Send + use<> {
        // Toute copie (même sans Règle) est enregistrée : elle annule celle en cours.
        let registration = self.state.lock().unwrap().register(text);
        let pipeline = self
            .config
            .read()
            .unwrap()
            .active
            .rule_for(text)
            .map(Rule::run);
        let state = Arc::clone(&self.state);
        let text = text.to_owned();
        async move {
            let (ticket, superseded) = match registration {
                Registration::OwnWrite => return Outcome::Nothing(Skip::OwnWrite),
                Registration::Accepted(ticket, superseded) => (ticket, superseded),
            };
            let Some(pipeline) = pipeline else {
                return Outcome::Nothing(Skip::NoRule);
            };
            // Une copie plus récente arrête le Pipeline sur place : l'étape en
            // cours est abandonnée (processus d'une `command` tué, requête
            // `youtube` coupée) et les suivantes ne tournent pas.
            let result = tokio::select! {
                result = pipeline.run(&text) => result,
                () = superseded.wait() => return Outcome::Cancelled,
            };
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
