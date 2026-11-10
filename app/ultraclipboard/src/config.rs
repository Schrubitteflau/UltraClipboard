//! Fichier de config de l'utilisateur : où il est, et ce qu'on charge au lancement.

use std::io;
use std::path::{Path, PathBuf};

use engine::{Config, SAMPLE_CONFIG};

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

/// Config à suivre au lancement.
///
/// Fichier absent : il est créé à partir de la config d'exemple, puis chargé.
/// Fichier illisible ou invalide : l'erreur est journalisée et l'appli démarre
/// sans aucune Règle, sans toucher au fichier (il n'y a pas encore d'ancienne
/// config à garder, et refuser de démarrer ferait boucler le service).
pub fn startup_config(path: &Path) -> Config {
    match read_or_create(path) {
        Ok(yaml) => match Config::parse(&yaml) {
            Ok(config) => {
                eprintln!(
                    "ultraclipboard: config chargée ({} Règle(s)) : {}",
                    config.rules().len(),
                    path.display()
                );
                config
            }
            Err(err) => {
                eprintln!(
                    "ultraclipboard: config invalide, aucune Règle active : {} : {err}",
                    path.display()
                );
                Config::default()
            }
        },
        Err(err) => {
            eprintln!(
                "ultraclipboard: config illisible, aucune Règle active : {} : {err}",
                path.display()
            );
            Config::default()
        }
    }
}

fn read_or_create(path: &Path) -> io::Result<String> {
    match std::fs::read_to_string(path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            match create_sample(path) {
                Ok(()) => {
                    eprintln!(
                        "ultraclipboard: config d'exemple créée : {}",
                        path.display()
                    );
                    Ok(SAMPLE_CONFIG.to_owned())
                }
                // Apparu entre-temps : c'est celui de l'utilisateur.
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    std::fs::read_to_string(path)
                }
                Err(err) => Err(err),
            }
        }
        result => result,
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
