//! Moteur d'UltraClipboard : décide quoi écrire dans le presse-papier après une copie.
//!
//! Le moteur ne connaît ni D-Bus ni l'interface : l'appli lui signale les copies
//! et applique l'[`Outcome`] qu'il rend.
//!
//! Ce que fait le moteur est décrit par une [`Config`] : la première Règle dont le
//! motif correspond à la copie applique son Pipeline. La config active peut être
//! remplacée à tout moment ([`Engine::load`]) sans perdre l'état du moteur.

mod config;
mod history;
mod notify;
mod pipeline;
mod state;
mod transformations;

use std::future::Future;
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

use serde::Serialize;
use tokio::sync::{mpsc, watch};

pub use config::{Config, ConfigError, NamedPipeline, Notifications, Rule, SAMPLE_CONFIG, Step};
pub use history::{HISTORY_CAP, HistoryEntry, HistoryStatus, TEXT_CAP};
pub use notify::Notification;
pub use pipeline::Failure;
pub use transformations::Endpoints;

use history::History;
use notify::Notifier;
use state::{Registration, State, Ticket};

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
    /// UltraClipboard est en pause ([`Engine::set_paused`]) : la copie reste
    /// telle quelle.
    Paused,
}

/// Ce qui est chargé, pour l'afficher : la config active et le refus du dernier
/// chargement, s'il a échoué (la config active est alors la précédente).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigState {
    /// Règles de la config active, dans l'ordre du fichier.
    pub rules: Vec<RuleState>,
    /// Pipelines de la config active, dans l'ordre du fichier.
    pub pipelines: Vec<NamedPipeline>,
    /// Faux tant qu'aucune config n'a été acceptée (config refusée dès le
    /// lancement) : aucune Règle n'est active.
    pub loaded: bool,
    /// Refus du dernier chargement ; `None` s'il a réussi.
    pub error: Option<Refusal>,
}

/// Chargement refusé : pourquoi, et d'où vient le refus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    pub kind: RefusalKind,
    #[serde(flatten)]
    pub error: ConfigError,
}

/// D'où vient un refus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RefusalKind {
    /// Le fichier est absent ou illisible ([`Engine::reject_config`]).
    Unreadable,
    /// Le contenu est invalide ([`Engine::load`]).
    Invalid,
}

/// Une Règle de la config active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleState {
    /// Expression régulière telle qu'écrite.
    pub pattern: String,
    /// Nom du Pipeline appliqué.
    pub pipeline: String,
}

/// Réagit aux copies. Bon marché à cloner : les clones partagent le même état.
#[derive(Clone)]
pub struct Engine {
    config: Arc<RwLock<Loaded>>,
    /// Signalé à chaque chargement, accepté ou refusé.
    changes: Arc<watch::Sender<()>>,
    state: Arc<Mutex<State>>,
    /// Hors de la config : un rechargement ne le touche pas.
    history: Arc<Mutex<History>>,
    notifier: Arc<Notifier>,
    /// Vrai en pause ; signalé à chaque bascule.
    paused: Arc<watch::Sender<bool>>,
    /// Abonnés aux écritures ([`Engine::writes`]). Toujours verrouillé sous le
    /// verrou de `state`, jamais l'inverse.
    writes: Arc<Writes>,
}

/// Config active et refus du dernier chargement, s'il a échoué.
struct Loaded {
    active: Arc<Config>,
    /// Faux tant qu'aucune config n'a été acceptée.
    loaded: bool,
    error: Option<Refusal>,
}

impl Default for Engine {
    /// Moteur sans config chargée (au lancement, avant de lire le fichier) :
    /// aucune Règle, aucune copie transformée.
    fn default() -> Self {
        Self::with(Loaded {
            active: Arc::default(),
            loaded: false,
            error: None,
        })
    }
}

impl Engine {
    /// Moteur qui suit cette config, comptée comme chargée. `Config::default()` :
    /// aucune Règle, aucune copie transformée. Sans config encore lue :
    /// [`Engine::default`].
    pub fn new(config: Config) -> Self {
        Self::with(Loaded {
            active: Arc::new(config),
            loaded: true,
            error: None,
        })
    }

    fn with(loaded: Loaded) -> Self {
        Self {
            config: Arc::new(RwLock::new(loaded)),
            changes: Arc::new(watch::Sender::new(())),
            state: Arc::default(),
            history: Arc::default(),
            notifier: Arc::default(),
            paused: Arc::new(watch::Sender::new(false)),
            writes: Arc::default(),
        }
    }

    /// Charge une config YAML et la rend active pour les copies suivantes.
    ///
    /// Une copie déjà en cours finit avec le Pipeline qu'elle avait choisi. L'état
    /// du moteur (anti-boucle, copie en cours) n'est pas touché. Config refusée :
    /// l'active reste en place et l'erreur est retenue ([`Engine::config_error`]).
    pub fn load(&self, yaml: &str) -> Result<(), ConfigError> {
        let parsed = Config::parse(yaml);
        let result = {
            let mut loaded = self.config.write().unwrap();
            match parsed {
                Ok(config) => {
                    loaded.active = Arc::new(config);
                    loaded.loaded = true;
                    loaded.error = None;
                    Ok(())
                }
                Err(err) => {
                    loaded.error = Some(Refusal {
                        kind: RefusalKind::Invalid,
                        error: err.clone(),
                    });
                    Err((err, loaded.active.notifications()))
                }
            }
        };
        self.changes.send_replace(());
        result.map_err(|(err, level)| {
            self.notifier
                .send(level, Notification::ConfigRejected(err.clone()));
            err
        })
    }

    /// La config n'a pas pu être lue (fichier illisible…) : l'active reste en
    /// place et cette erreur est retenue comme celle d'un chargement refusé.
    pub fn reject_config(&self, error: ConfigError) {
        let level = {
            let mut loaded = self.config.write().unwrap();
            loaded.error = Some(Refusal {
                kind: RefusalKind::Unreadable,
                error: error.clone(),
            });
            loaded.active.notifications()
        };
        self.changes.send_replace(());
        self.notifier
            .send(level, Notification::ConfigRejected(error));
    }

    /// Ce qui est chargé : Règles et Pipelines actifs, refus du dernier chargement.
    pub fn config_state(&self) -> ConfigState {
        let loaded = self.config.read().unwrap();
        ConfigState {
            rules: loaded
                .active
                .rules()
                .iter()
                .map(|rule| RuleState {
                    pattern: rule.pattern().to_owned(),
                    pipeline: rule.pipeline().to_owned(),
                })
                .collect(),
            pipelines: loaded.active.pipelines().to_vec(),
            loaded: loaded.loaded,
            error: loaded.error.clone(),
        }
    }

    /// Reçoit un signal à chaque chargement de config qui suit cet appel, accepté
    /// ou refusé : de quoi relire [`Engine::config_state`]. Des chargements
    /// rapprochés peuvent n'en donner qu'un.
    pub fn config_changes(&self) -> watch::Receiver<()> {
        self.changes.subscribe()
    }

    /// Config active.
    pub fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config.read().unwrap().active)
    }

    /// Pourquoi le dernier chargement a été refusé ; `None` s'il a réussi (ou s'il
    /// n'y en a pas eu).
    pub fn config_error(&self) -> Option<ConfigError> {
        let loaded = self.config.read().unwrap();
        loaded.error.as_ref().map(|refusal| refusal.error.clone())
    }

    /// S'abonne aux notifications que le moteur décide d'envoyer à partir de
    /// maintenant. L'appli les montre telles quelles.
    pub fn subscribe_notifications(&self) -> tokio::sync::broadcast::Receiver<Notification> {
        self.notifier.subscribe()
    }

    /// Coupe (ou rétablit) toutes les notifications jusqu'au redémarrage, quel
    /// que soit le niveau de la config. Rétablies, elles suivent de nouveau ce
    /// niveau.
    pub fn set_notifications_muted(&self, muted: bool) {
        self.notifier.set_muted(muted);
    }

    /// Vrai si les notifications sont coupées pour la session.
    pub fn notifications_muted(&self) -> bool {
        self.notifier.muted()
    }

    /// Met en pause (ou reprend) jusqu'au redémarrage : en pause, aucune copie
    /// n'est transformée ([`Skip::Paused`]).
    ///
    /// Mettre en pause arrête le traitement en cours, s'il y en a un (copie ou
    /// réapplication) : son issue est [`Outcome::Cancelled`], rien n'est écrit,
    /// il entre dans l'historique comme annulé, sans notification. Mettre en
    /// pause ne change rien à ce qui est restaurable. Une copie faite en pause reste une copie : elle
    /// arrête le traitement en cours, et l'Original d'avant elle n'est plus
    /// restauré (la dernière copie gagne). Restaurer l'Original et réappliquer
    /// ([`Engine::reapply`]) marchent en pause.
    pub fn set_paused(&self, paused: bool) {
        // Sous le verrou de l'état : une copie enregistrée avant la pause est
        // arrêtée, une copie enregistrée après voit la pause.
        let mut state = self.state.lock().unwrap();
        let changed = self.paused.send_if_modified(|current| {
            let changed = *current != paused;
            *current = paused;
            changed
        });
        if changed && paused {
            state.cancel();
        }
    }

    /// Textes à écrire dans le presse-papier (résultats et restaurations),
    /// chacun publié au moment où le moteur décide de l'écrire, sous le verrou
    /// de son état : l'ordre de réception est l'ordre des décisions, et une
    /// copie dépassée ne publie rien. Chaque abonné reçoit toutes les écritures
    /// décidées après l'appel, sans en perdre : chacune est attendue en écho par
    /// l'anti-boucle, elle doit donc partir. L'abonné qui les écrit doit le
    /// faire dans l'ordre reçu, et n'avoir aucune autre source d'écriture.
    pub fn writes(&self) -> mpsc::UnboundedReceiver<String> {
        let (sender, receiver) = mpsc::unbounded_channel();
        // Sous le verrou de l'état, comme une publication.
        let _state = self.state.lock().unwrap();
        self.writes.0.lock().unwrap().push(sender);
        receiver
    }

    /// Vrai en pause.
    pub fn paused(&self) -> bool {
        *self.paused.borrow()
    }

    /// Reçoit un signal à chaque mise en pause ou reprise qui suit cet appel :
    /// de quoi relire [`Engine::paused`].
    pub fn pause_changes(&self) -> watch::Receiver<bool> {
        self.paused.subscribe()
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
        let (registration, paused) = {
            let mut state = self.state.lock().unwrap();
            (state.register(text), self.paused())
        };
        self.process(text, registration, paused)
    }

    /// Réapplique les Règles à `text` (le contenu actuel du presse-papier) :
    /// même issue qu'une copie de ce texte ([`Engine::on_copy`] : annule la
    /// copie en cours, entre dans l'historique, notifie ; son texte devient
    /// l'Original restaurable si elle applique une Transformation), mais c'est
    /// une action explicite : elle marche en pause et transforme même un texte
    /// écrit par UltraClipboard (pas d'anti-boucle). Ce n'est pas une copie :
    /// sans Transformation appliquée, l'Original d'avant reste restaurable, et
    /// les échos de nos écritures précédentes restent attendus.
    pub fn reapply(&self, text: &str) -> impl Future<Output = Outcome> + Send + use<> {
        let registration = self.state.lock().unwrap().register_explicit();
        self.process(text, registration, false)
    }

    /// Traitement d'une copie enregistrée ; `paused` : ne rien transformer.
    fn process(
        &self,
        text: &str,
        registration: Registration,
        paused: bool,
    ) -> impl Future<Output = Outcome> + Send + use<> {
        let at = SystemTime::now();
        let matched = self
            .config
            .read()
            .unwrap()
            .active
            .rule_for(text)
            .filter(|_| !paused)
            .map(|rule| {
                let pipeline = rule.run();
                (
                    rule.pattern().to_owned(),
                    rule.pipeline().to_owned(),
                    pipeline,
                )
            });
        let state = Arc::clone(&self.state);
        let history = Arc::clone(&self.history);
        let notifier = Arc::clone(&self.notifier);
        let config = Arc::clone(&self.config);
        let writes = Arc::clone(&self.writes);
        let text = text.to_owned();
        async move {
            let (ticket, superseded) = match registration {
                Registration::OwnWrite => return Outcome::Nothing(Skip::OwnWrite),
                Registration::Accepted(ticket, superseded) => (ticket, superseded),
            };
            if paused {
                return Outcome::Nothing(Skip::Paused);
            }
            let Some((rule, name, pipeline)) = matched else {
                return Outcome::Nothing(Skip::NoRule);
            };
            // Une copie plus récente arrête le Pipeline sur place : l'étape en
            // cours est abandonnée (processus d'une `command` tué, requête
            // `youtube` coupée) et les suivantes ne tournent pas.
            let result = tokio::select! {
                result = pipeline.run(&text) => Some(result),
                () = superseded.wait() => None,
            };
            let outcome = settle(&state, &writes, ticket, &text, result);
            // Le niveau de la config active quand le Pipeline finit.
            let level = config.read().unwrap().active.notifications();
            match &outcome {
                Outcome::Write(_) => notifier.send(
                    level,
                    Notification::Succeeded {
                        pipeline: name.clone(),
                    },
                ),
                Outcome::Failed(failure) => notifier.send(
                    level,
                    Notification::Failed {
                        pipeline: name.clone(),
                        failure: failure.clone(),
                    },
                ),
                Outcome::Nothing(Skip::EmptyResult) => notifier.send(
                    level,
                    Notification::EmptyResult {
                        pipeline: name.clone(),
                    },
                ),
                Outcome::Nothing(_) | Outcome::Cancelled => {}
            }
            // Annulée, échouée ou réussie : la copie entre dans l'historique.
            if let Some(status) = HistoryStatus::of(&outcome, &text) {
                let entry = HistoryEntry {
                    at,
                    rule,
                    pipeline: name,
                    original: text,
                    status,
                };
                history.lock().unwrap().record(ticket, entry);
            }
            outcome
        }
    }

    /// Restaure l'Original : rend le texte à écrire pour annuler la dernière
    /// Transformation appliquée. Sans Original : rien n'est écrit (`None`).
    ///
    /// Le texte rendu compte comme notre écriture : son écho est ignoré une
    /// fois, il n'est pas retransformé, même s'il revient après l'écho du
    /// résultat ou d'une autre restauration. Restaurer de nouveau le réécrit.
    ///
    /// La dernière copie gagne : l'Original ne remplace jamais une copie plus
    /// récente que la Transformation, et rien n'est écrit. Une copie en cours
    /// de traitement (qu'il y ait un Original ou non) s'arrête, son issue est
    /// [`Outcome::Cancelled`], et elle reste telle que copiée. Une
    /// réapplication en cours s'arrête aussi, mais ce n'est pas une copie :
    /// l'Original d'avant elle est restauré. Ni historique ni notification :
    /// seule une copie y entre.
    pub fn restore_original(&self) -> Option<String> {
        let mut state = self.state.lock().unwrap();
        let original = state.restore();
        if let Some(original) = &original {
            self.writes.publish(original);
        }
        original
    }

    /// Les dernières transformations ([`HISTORY_CAP`] au plus), la plus récente
    /// en tête : une entrée par copie qui a déclenché un Pipeline. En mémoire
    /// seulement ; conservé quand la config change.
    pub fn history(&self) -> Vec<HistoryEntry> {
        self.history.lock().unwrap().entries()
    }

    /// Reçoit un signal à chaque entrée ajoutée à l'historique après cet appel :
    /// de quoi relire [`Engine::history`]. Des ajouts rapprochés peuvent n'en
    /// donner qu'un.
    pub fn history_changes(&self) -> watch::Receiver<()> {
        self.history.lock().unwrap().subscribe()
    }
}

/// Issue d'un Pipeline qui a tourné jusqu'au bout (`Some`) ou a été arrêté par
/// une copie plus récente (`None`).
fn settle(
    state: &Mutex<State>,
    writes: &Writes,
    ticket: Ticket,
    text: &str,
    result: Option<Result<String, Failure>>,
) -> Outcome {
    let mut state = state.lock().unwrap();
    let Some(result) = result.filter(|_| state.is_current(ticket)) else {
        return Outcome::Cancelled;
    };
    match result {
        Err(failure) => Outcome::Failed(failure),
        Ok(output) if output == text => Outcome::Nothing(Skip::Unchanged),
        Ok(output) if output.is_empty() => Outcome::Nothing(Skip::EmptyResult),
        Ok(output) => {
            state.transformed(ticket, text, &output);
            // Sous le verrou : une copie plus récente ne peut ni passer devant
            // ce résultat ni être recouverte par lui.
            writes.publish(&output);
            Outcome::Write(output)
        }
    }
}

/// Abonnés aux écritures décidées ([`Engine::writes`]).
#[derive(Default)]
struct Writes(Mutex<Vec<mpsc::UnboundedSender<String>>>);

impl Writes {
    /// Publie `text` à chaque abonné encore là ; à appeler sous le verrou de
    /// l'état, au moment où l'écriture est décidée.
    fn publish(&self, text: &str) {
        self.0
            .lock()
            .unwrap()
            .retain(|sender| sender.send(text.to_owned()).is_ok());
    }
}
