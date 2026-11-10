//! Fichier de config de l'utilisateur : où il est, son chargement au lancement et
//! son rechargement à chaque enregistrement.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::time::Instant;

use engine::{ConfigError, Engine, SAMPLE_CONFIG};
use notify::event::{AccessKind, AccessMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

/// `$XDG_CONFIG_HOME/ultraclipboard/config.yaml`, ou `~/.config/…` quand
/// `XDG_CONFIG_HOME` est absent, vide ou relatif (spécification XDG). `None`
/// si ni lui ni `HOME` ne sont utilisables.
pub fn config_path() -> Option<PathBuf> {
    let absolute = |var| {
        std::env::var_os(var)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    let base =
        absolute("XDG_CONFIG_HOME").or_else(|| absolute("HOME").map(|h| h.join(".config")))?;
    Some(base.join("ultraclipboard").join("config.yaml"))
}

/// Moteur de l'appli : aucune config chargée tant que le fichier n'est pas lu,
/// puis il charge `path` et le suit ([`follow_config`]). Sans `path` (ni
/// XDG_CONFIG_HOME ni HOME), il reste sans Règle.
///
/// À appeler dans un runtime tokio.
pub fn start_engine(path: Option<PathBuf>) -> (Engine, Option<ConfigFollower>) {
    let engine = Engine::default();
    let follower = match path {
        Some(path) => Some(follow_config(path, engine.clone())),
        None => {
            eprintln!("ultraclipboard: ni XDG_CONFIG_HOME ni HOME, aucune Règle active");
            None
        }
    };
    (engine, follower)
}

/// Délai sans nouvel évènement sur le fichier avant de le relire : un éditeur
/// enregistre parfois en plusieurs temps (troncature puis écriture, renommage
/// puis création), seul l'état final compte.
const QUIET: Duration = Duration::from_millis(100);

/// Liens symboliques suivis au plus, de lien en lien, jusqu'au fichier.
const MAX_LINKS: usize = 16;

/// Suit le fichier de config tant qu'il vit ; l'arrête quand on le lâche.
pub struct ConfigFollower {
    task: tokio::task::JoinHandle<()>,
}

impl Drop for ConfigFollower {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Charge la config dans `engine`, puis la recharge à chaque enregistrement.
///
/// Au lancement, fichier absent : il est créé à partir de la config d'exemple
/// (un lien symbolique dont la cible manque n'est pas remplacé).
///
/// Config invalide, fichier illisible ou absent : refusé, la config active reste
/// en place (aucune Règle au lancement) et le moteur retient l'erreur jusqu'au
/// prochain chargement réussi ; le fichier n'est jamais réécrit. Chaque
/// enregistrement relit le fichier : un texte invalide est refusé (et signalé) de
/// nouveau, seul un texte identique à la config active n'est pas rechargé.
///
/// `path` peut être un lien symbolique, ou se trouver dans un dossier qui en est
/// un (stow) : les enregistrements de la cible sont suivis, et la surveillance
/// suit le lien quand il est redirigé, le dossier quand il est supprimé puis
/// recréé, la cible quand elle apparaît.
///
/// À appeler dans un runtime tokio : la surveillance y tourne en tâche de fond.
pub fn follow_config(path: PathBuf, engine: Engine) -> ConfigFollower {
    if let Err(err) = create_if_missing(&path) {
        eprintln!(
            "ultraclipboard: config d'exemple impossible à créer : {} : {err}",
            path.display()
        );
    }
    // Surveiller avant de lire : un enregistrement entre les deux n'est pas perdu.
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut watch = Watch::start(&path, tx)
        .inspect_err(|err| {
            eprintln!(
                "ultraclipboard: surveillance de la config impossible, pas de rechargement à chaud : {err}"
            )
        })
        .ok();
    let mut loader = Loader {
        engine,
        path,
        active: None,
    };
    loader.reload();
    let task = tokio::spawn(async move {
        let Some(watch) = watch.as_mut() else { return };
        loop {
            // Attendre un évènement qui concerne le fichier, puis le calme.
            loop {
                match rx.recv().await {
                    None => return,
                    Some(paths) if watch.concerns(&paths) => break,
                    Some(_) => {}
                }
            }
            let mut deadline = Instant::now() + QUIET;
            loop {
                match tokio::time::timeout_at(deadline, rx.recv()).await {
                    Ok(None) => return,
                    Ok(Some(paths)) if watch.concerns(&paths) => deadline = Instant::now() + QUIET,
                    Ok(Some(_)) => {}
                    Err(_) => break,
                }
            }
            // Le dossier ou un lien a pu changer : surveiller les nouveaux, puis lire.
            watch.refresh();
            loader.reload();
        }
    });
    ConfigFollower { task }
}

/// Lit le fichier et en charge le texte dans le moteur.
struct Loader {
    engine: Engine,
    path: PathBuf,
    /// Texte de la config active tant qu'aucun refus ne l'a suivi : le relire à
    /// l'identique n'a rien à changer.
    active: Option<String>,
}

impl Loader {
    fn reload(&mut self) {
        let path = self.path.display();
        // Au lancement, un refus laisse le moteur sans aucune Règle.
        let kept = if self.engine.config().rules().is_empty() && self.active.is_none() {
            "aucune Règle active"
        } else {
            "la config active reste en place"
        };
        let yaml = match std::fs::read_to_string(&self.path) {
            Ok(yaml) => yaml,
            Err(err) => {
                let reason = match err.kind() {
                    io::ErrorKind::NotFound => absent(&self.path),
                    _ => format!("fichier illisible : {err}"),
                };
                eprintln!("ultraclipboard: config refusée, {kept} : {path} : {reason}");
                self.engine.reject_config(ConfigError {
                    field: String::new(),
                    reason,
                });
                self.active = None;
                return;
            }
        };
        if self.active.as_deref() == Some(yaml.as_str()) {
            return;
        }
        match self.engine.load(&yaml) {
            Ok(()) => {
                eprintln!(
                    "ultraclipboard: config chargée ({} Règle(s)) : {path}",
                    self.engine.config().rules().len()
                );
                self.active = Some(yaml);
            }
            Err(err) => {
                eprintln!("ultraclipboard: config invalide, {kept} : {path} : {err}");
                self.active = None;
            }
        }
    }
}

/// Raison d'un fichier introuvable, en nommant la cible d'un lien qui ne mène nulle part.
fn absent(path: &Path) -> String {
    match std::fs::read_link(path) {
        Ok(target) => format!(
            "fichier absent (le lien symbolique mène à {}, introuvable)",
            target.display()
        ),
        Err(_) => "fichier absent".to_owned(),
    }
}

/// Surveillance des dossiers dont dépend le fichier.
///
/// Pour le fichier et chaque cible de lien symbolique : son dossier (ou, s'il
/// n'existe pas, le plus proche ancêtre existant) et le parent de celui-ci, qui
/// voit le dossier supprimé, recréé ou remplacé par un lien. Les dossiers et non
/// le fichier : un enregistrement par renommage remplace le fichier.
struct Watch {
    watcher: RecommendedWatcher,
    path: PathBuf,
    /// Le fichier, puis chaque cible de lien jusqu'au vrai fichier.
    chain: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

impl Watch {
    fn start(path: &Path, tx: mpsc::UnboundedSender<Vec<PathBuf>>) -> notify::Result<Self> {
        let watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            let Ok(event) = event else { return };
            // Nos propres lectures du fichier produisent des évènements d'accès.
            if matches!(event.kind, EventKind::Access(kind) if kind != AccessKind::Close(AccessMode::Write))
            {
                return;
            }
            let _ = tx.send(event.paths);
        })?;
        let mut watch = Self {
            watcher,
            path: path.to_owned(),
            chain: Vec::new(),
            dirs: Vec::new(),
        };
        watch.refresh();
        Ok(watch)
    }

    /// Vrai si l'évènement touche le fichier, une cible de lien ou l'un de leurs dossiers.
    fn concerns(&self, paths: &[PathBuf]) -> bool {
        paths
            .iter()
            .any(|p| self.chain.iter().any(|file| file.starts_with(p)))
    }

    /// Recalcule les dossiers à surveiller et les surveille à nouveau, tous : un
    /// dossier recréé ou un lien redirigé garde son chemin mais plus sa cible.
    fn refresh(&mut self) {
        for dir in self.dirs.drain(..) {
            let _ = self.watcher.unwatch(&dir);
        }
        self.chain = chain(&self.path);
        for file in &self.chain {
            let Some(mut dir) = file.parent().map(Path::to_path_buf) else {
                continue;
            };
            while !dir.is_dir() {
                match dir.parent() {
                    Some(parent) => dir = parent.to_path_buf(),
                    None => break,
                }
            }
            let parent = dir.parent().map(Path::to_path_buf);
            for dir in std::iter::once(dir).chain(parent) {
                if !self.dirs.contains(&dir) {
                    match self.watcher.watch(&dir, RecursiveMode::NonRecursive) {
                        Ok(()) => self.dirs.push(dir),
                        Err(err) => eprintln!(
                            "ultraclipboard: surveillance impossible : {} : {err}",
                            dir.display()
                        ),
                    }
                }
            }
        }
    }
}

/// Le fichier, puis chaque cible de lien symbolique, puis le chemin réel quand
/// un dossier du chemin est lui-même un lien.
fn chain(path: &Path) -> Vec<PathBuf> {
    let mut files = vec![path.to_owned()];
    while files.len() <= MAX_LINKS {
        let last = files.last().unwrap();
        let Ok(target) = std::fs::read_link(last) else {
            break;
        };
        let target = match last.parent() {
            Some(dir) => dir.join(target),
            None => target,
        };
        files.push(target);
    }
    if let Ok(real) = std::fs::canonicalize(path)
        && !files.contains(&real)
    {
        files.push(real);
    }
    files
}

/// Crée la config d'exemple si rien n'existe à cet emplacement.
fn create_if_missing(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        other => return other.map(|_| ()),
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    match create_sample(path) {
        Ok(()) => {
            eprintln!(
                "ultraclipboard: config d'exemple créée : {}",
                path.display()
            );
            Ok(())
        }
        // Apparu entre-temps : c'est celui de l'utilisateur.
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(err) => Err(err),
    }
}

/// Écrit l'exemple dans un fichier temporaire du même dossier, puis le publie
/// sous `path` par un lien physique : un fichier existant n'est jamais écrasé, et
/// une écriture interrompue (disque plein) ne laisse pas de config vide derrière elle.
fn create_sample(path: &Path) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);
    let result = std::fs::write(&tmp, SAMPLE_CONFIG).and_then(|()| std::fs::hard_link(&tmp, path));
    let _ = std::fs::remove_file(&tmp);
    result
}
