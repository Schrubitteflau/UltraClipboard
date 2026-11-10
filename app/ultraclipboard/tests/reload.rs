//! Rechargement à chaud : enregistrer le fichier de config change le résultat des
//! `Copied` suivants, comme le font les éditeurs (remplacement par renommage,
//! écriture en deux temps, fichier supprimé puis recréé).

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{Extension, PrivateBus};
use engine::{Config, Engine};
use ultraclipboard::{follow_config, serve};

const SOON: Duration = Duration::from_secs(5);
const NONE_WITHIN: Duration = Duration::from_millis(300);
/// Plus long que le temps de rechargement, pour vérifier qu'un état ne change pas.
const SETTLE: Duration = Duration::from_millis(600);

const UPPER: &str =
    "config_version: 1\npipelines:\n  p: [upper]\nrules:\n  - match: ''\n    pipeline: p\n";
const LOWER: &str =
    "config_version: 1\npipelines:\n  p: [lower]\nrules:\n  - match: ''\n    pipeline: p\n";
const BROKEN: &str = "config_version: 1\nrules:\n  - match: ''\n    pipeline: absent\n";

/// Dossier vide propre au test, dans le dossier temporaire de cargo.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("reload-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Enregistrement « atomique » : fichier temporaire voisin, puis renommage.
fn save_by_rename(path: &Path, yaml: &str) {
    let tmp = path.with_extension("yaml.new");
    std::fs::write(&tmp, yaml).unwrap();
    std::fs::rename(&tmp, path).unwrap();
}

struct App {
    engine: Engine,
    extension: Extension,
    _bus: PrivateBus,
    _app: zbus::Connection,
    _follower: ultraclipboard::ConfigFollower,
}

async fn start(path: &Path, yaml: &str) -> App {
    std::fs::write(path, yaml).unwrap();
    follow(path).await
}

/// Lance l'appli sur le fichier tel qu'il est (le test l'a préparé).
async fn follow(path: &Path) -> App {
    let bus = PrivateBus::start();
    let app = bus.connect().await;
    let engine = Engine::new(Config::default());
    let follower = follow_config(path.to_owned(), engine.clone());
    serve(&app, engine.clone()).await.unwrap();
    let extension = Extension::connect(&bus).await;
    App {
        engine,
        extension,
        _bus: bus,
        _app: app,
        _follower: follower,
    }
}

impl App {
    /// Attend que le moteur retienne une erreur de config, ou échoue après [`SOON`].
    async fn eventually_refused(&self) -> engine::ConfigError {
        let deadline = Instant::now() + SOON;
        while Instant::now() < deadline {
            if let Some(err) = self.engine.config_error() {
                return err;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("aucune erreur de config retenue");
    }

    /// Copie `text` jusqu'à recevoir `want`, ou échoue après [`SOON`].
    async fn eventually_applies(&mut self, text: &str, want: &str) {
        let deadline = Instant::now() + SOON;
        while Instant::now() < deadline {
            self.extension.copied(text).await;
            if self.extension.next_apply(NONE_WITHIN).await.as_deref() == Some(want) {
                return;
            }
        }
        panic!("« {text} » n'a jamais donné « {want} »");
    }

    async fn applies(&mut self, text: &str) -> Option<String> {
        self.extension.copied(text).await;
        self.extension.next_apply(SOON).await
    }
}

#[tokio::test]
async fn saving_by_rename_changes_the_next_copies() {
    let path = scratch("rename").join("config.yaml");
    let mut app = start(&path, UPPER).await;
    assert_eq!(app.applies("Ab").await.as_deref(), Some("AB"));

    save_by_rename(&path, LOWER);
    app.eventually_applies("Cd", "cd").await;
    // Et ça continue de suivre les enregistrements suivants.
    save_by_rename(&path, UPPER);
    app.eventually_applies("Ef", "EF").await;
}

/// Troncature puis écriture, comme un éditeur qui réécrit le fichier en place :
/// le fichier vide entre les deux n'est jamais lu comme une config.
#[tokio::test]
async fn saving_in_two_writes_never_reports_the_half_written_file() {
    let path = scratch("two-writes").join("config.yaml");
    let mut app = start(&path, UPPER).await;
    let engine = app.engine.clone();
    let watch = tokio::spawn(async move {
        loop {
            if let Some(err) = engine.config_error() {
                return err;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    });

    std::fs::write(&path, "").unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    std::fs::write(&path, LOWER).unwrap();
    app.eventually_applies("Cd", "cd").await;
    tokio::time::sleep(SETTLE).await;
    assert!(!watch.is_finished(), "erreur vue : {:?}", watch.await);
    watch.abort();
}

/// Copie de sauvegarde par renommage, puis nouveau fichier (vim sans `backupcopy`).
#[tokio::test]
async fn a_removed_file_is_reported_and_followed_when_it_returns() {
    let path = scratch("recreate").join("config.yaml");
    let mut app = start(&path, UPPER).await;

    std::fs::rename(&path, path.with_extension("yaml~")).unwrap();
    let err = app.eventually_refused().await;
    assert_eq!(err.field, "");
    assert!(err.reason.contains("absent"), "{err}");
    // La config active reste en place.
    assert_eq!(app.applies("Ab").await.as_deref(), Some("AB"));

    // Le même texte revient : rechargé, l'erreur disparaît.
    std::fs::write(&path, UPPER).unwrap();
    let deadline = Instant::now() + SOON;
    while app.engine.config_error().is_some() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(app.engine.config_error(), None);
    save_by_rename(&path, LOWER);
    app.eventually_applies("Cd", "cd").await;
}

#[tokio::test]
async fn a_removed_then_recreated_directory_is_followed() {
    let dir = scratch("dir-recreate").join("ultraclipboard");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.yaml");
    let mut app = start(&path, UPPER).await;

    std::fs::remove_dir_all(&dir).unwrap();
    assert!(app.eventually_refused().await.reason.contains("absent"));
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(&path, LOWER).unwrap();
    app.eventually_applies("Cd", "cd").await;
    assert_eq!(app.engine.config_error(), None);
    save_by_rename(&path, UPPER);
    app.eventually_applies("Ef", "EF").await;
}

/// Enregistrer de nouveau un fichier invalide inchangé le refuse de nouveau (pour
/// le signaler à nouveau) ; seul un texte valide inchangé n'est pas rechargé.
#[tokio::test]
async fn re_saving_an_unchanged_invalid_file_refuses_it_again() {
    let path = scratch("invalid-again").join("config.yaml");
    let mut app = start(&path, BROKEN).await;
    assert_eq!(app.eventually_refused().await.field, "rules[0].pipeline");

    // Le moteur charge une config d'ailleurs : l'erreur disparaît…
    app.engine.load(UPPER).unwrap();
    assert_eq!(app.engine.config_error(), None);
    // … et le même texte invalide, enregistré de nouveau, est refusé de nouveau.
    save_by_rename(&path, BROKEN);
    assert_eq!(app.eventually_refused().await.field, "rules[0].pipeline");
    assert_eq!(app.applies("Ab").await.as_deref(), Some("AB"));
}

/// D36 : invalide au lancement (aucune Règle), puis corrigée.
#[tokio::test]
async fn an_invalid_config_at_startup_is_loaded_once_fixed() {
    let path = scratch("startup-fixed").join("config.yaml");
    let mut app = start(&path, BROKEN).await;
    assert_eq!(app.eventually_refused().await.field, "rules[0].pipeline");
    app.extension.copied("Ab").await;
    assert_eq!(app.extension.next_apply(NONE_WITHIN).await, None);

    save_by_rename(&path, LOWER);
    app.eventually_applies("Cd", "cd").await;
    assert_eq!(app.engine.config_error(), None);
}

#[tokio::test]
async fn an_invalid_save_keeps_the_active_config_until_fixed() {
    let path = scratch("invalid").join("config.yaml");
    let mut app = start(&path, UPPER).await;

    save_by_rename(&path, BROKEN);
    let deadline = Instant::now() + SOON;
    while app.engine.config_error().is_none() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        app.engine.config_error().map(|e| e.field),
        Some("rules[0].pipeline".to_owned())
    );
    assert_eq!(app.applies("Ab").await.as_deref(), Some("AB"));

    save_by_rename(&path, LOWER);
    app.eventually_applies("Cd", "cd").await;
    assert_eq!(app.engine.config_error(), None);
}

#[cfg(unix)]
mod symlinks {
    use super::*;
    use std::os::unix::fs::symlink;

    #[tokio::test]
    async fn a_symlinked_config_follows_saves_of_its_target() {
        let dir = scratch("symlink");
        let target = dir.join("dotfiles").join("ultraclipboard.yaml");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, UPPER).unwrap();
        let path = dir.join("config.yaml");
        symlink(&target, &path).unwrap();
        let mut app = follow(&path).await;
        assert_eq!(app.applies("Ab").await.as_deref(), Some("AB"));

        save_by_rename(&target, LOWER);
        app.eventually_applies("Cd", "cd").await;
    }

    /// `ln -sfn` vers un fichier d'un autre dossier : on suit le nouveau.
    #[tokio::test]
    async fn a_re_pointed_symlink_follows_its_new_target() {
        let dir = scratch("symlink-repoint");
        let (a, b) = (dir.join("a"), dir.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("config.yaml"), UPPER).unwrap();
        std::fs::write(b.join("config.yaml"), LOWER).unwrap();
        let path = dir.join("config.yaml");
        symlink(a.join("config.yaml"), &path).unwrap();
        let mut app = follow(&path).await;
        assert_eq!(app.applies("Ab").await.as_deref(), Some("AB"));

        let tmp = dir.join("config.yaml.link");
        symlink(b.join("config.yaml"), &tmp).unwrap();
        std::fs::rename(&tmp, &path).unwrap();
        app.eventually_applies("Cd", "cd").await;
        // Les enregistrements suivants de la nouvelle cible sont suivis.
        save_by_rename(&b.join("config.yaml"), UPPER);
        app.eventually_applies("Ef", "EF").await;
    }

    /// Dossier de config déplacé dans des dotfiles et remplacé par un lien (stow).
    #[tokio::test]
    async fn a_config_directory_replaced_by_a_symlink_is_followed() {
        let root = scratch("symlink-dir");
        let dir = root.join("ultraclipboard");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.yaml");
        let mut app = start(&path, UPPER).await;

        let stowed = root.join("dotfiles");
        std::fs::create_dir_all(&stowed).unwrap();
        std::fs::write(stowed.join("config.yaml"), LOWER).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        symlink(&stowed, &dir).unwrap();
        app.eventually_applies("Cd", "cd").await;
        save_by_rename(&stowed.join("config.yaml"), UPPER);
        app.eventually_applies("Ef", "EF").await;
    }

    /// Lien vers une cible absente au lancement : erreur visible, puis chargée
    /// quand la cible apparaît (sans remplacer le lien par l'exemple).
    #[tokio::test]
    async fn a_dangling_symlink_at_startup_is_reported_until_its_target_appears() {
        let dir = scratch("symlink-dangling");
        let target = dir.join("dotfiles").join("ultraclipboard.yaml");
        let path = dir.join("config.yaml");
        symlink(&target, &path).unwrap();
        let mut app = follow(&path).await;
        assert!(app.eventually_refused().await.reason.contains("absent"));
        assert!(path.is_symlink());

        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, LOWER).unwrap();
        app.eventually_applies("Cd", "cd").await;
        assert_eq!(app.engine.config_error(), None);
    }
}
