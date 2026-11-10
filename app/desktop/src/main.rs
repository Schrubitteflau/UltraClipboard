//! Appli UltraClipboard : un seul processus Tauri qui héberge le moteur, le service
//! D-Bus (pour l'extension GNOME Shell), l'icône de la barre du haut et la fenêtre.
//!
//! Le service D-Bus démarre avant toute interface : une seconde instance s'arrête
//! avec un message sans jamais ouvrir d'affichage.

use std::process::ExitCode;

use engine::{ConfigState, Engine, HistoryEntry};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Emitter, Manager, State, WindowEvent};
use ultraclipboard::{BUS_NAME, ConfigFollower, ServeError, serve, start_engine};

const MAIN_WINDOW: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";
/// Événement poussé à la fenêtre à chaque chargement de config ; porte un [`ConfigState`].
const CONFIG_CHANGED: &str = "config-changed";
/// Événement poussé à la fenêtre à chaque entrée d'historique ; porte l'historique entier.
const HISTORY_CHANGED: &str = "history-changed";

fn main() -> ExitCode {
    // Un seul runtime, partagé par le service D-Bus, le moteur et Tauri.
    let runtime = tokio::runtime::Runtime::new().expect("runtime tokio");
    let (conn, engine, _follower) = match runtime.block_on(start_service()) {
        Ok(started) => started,
        Err(err) => {
            eprintln!("ultraclipboard: {err}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("ultraclipboard: prêt sur {BUS_NAME}");
    tauri::async_runtime::set(runtime.handle().clone());

    tauri::Builder::default()
        // La connexion vit aussi longtemps que l'appli : le service avec elle.
        .manage(conn)
        .manage(engine)
        .invoke_handler(tauri::generate_handler![config_state, config_path, history])
        .setup(|app| {
            add_tray(app)?;
            push_config_changes(app);
            push_history_changes(app);
            Ok(())
        })
        // Fermer la fenêtre la cache ; seul « Quitter » arrête l'appli.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("démarrage de Tauri");
    ExitCode::SUCCESS
}

/// Charge la config (puis la suit à chaque enregistrement) et expose le moteur sur
/// le bus de session. Le suivi s'arrête quand on lâche le [`ConfigFollower`].
async fn start_service() -> Result<(zbus::Connection, Engine, Option<ConfigFollower>), ServeError> {
    let (engine, follower) = start_engine(ultraclipboard::config_path());
    let conn = zbus::connection::Builder::session().build().await?;
    serve(&conn, engine.clone()).await?;
    Ok((conn, engine, follower))
}

/// Ce qui est chargé (vue « Règles »).
#[tauri::command]
fn config_state(engine: State<'_, Engine>) -> ConfigState {
    engine.config_state()
}

/// Chemin du fichier de config ; `None` sans XDG_CONFIG_HOME ni HOME.
#[tauri::command]
fn config_path() -> Option<String> {
    ultraclipboard::config_path().map(|path| path.display().to_string())
}

/// Dernières transformations, la plus récente en tête (vue « Historique »).
#[tauri::command]
fn history(engine: State<'_, Engine>) -> Vec<HistoryEntry> {
    engine.history()
}

/// Pousse [`HISTORY_CHANGED`] à la fenêtre à chaque entrée d'historique.
fn push_history_changes(app: &App) {
    let engine = app.state::<Engine>().inner().clone();
    let mut changes = engine.history_changes();
    let app = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        while changes.changed().await.is_ok() {
            let _ = app.emit(HISTORY_CHANGED, engine.history());
        }
    });
}

/// Pousse [`CONFIG_CHANGED`] à la fenêtre à chaque chargement de config.
fn push_config_changes(app: &App) {
    let engine = app.state::<Engine>().inner().clone();
    let mut changes = engine.config_changes();
    let app = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        while changes.changed().await.is_ok() {
            let _ = app.emit(CONFIG_CHANGED, engine.config_state());
        }
    });
}

/// Icône AppIndicator. Sous Linux le clic sur l'icône n'émet rien : tout passe par le menu.
fn add_tray(app: &App) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, MENU_OPEN, "Ouvrir", true, None::<&str>)?,
            &MenuItem::with_id(app, MENU_QUIT, "Quitter", true, None::<&str>)?,
        ],
    )?;
    let mut tray = TrayIconBuilder::new()
        .tooltip("UltraClipboard")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_OPEN => show_window(app),
            MENU_QUIT => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
