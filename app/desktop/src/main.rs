//! Appli UltraClipboard : un seul processus Tauri qui héberge le moteur, le service
//! D-Bus (pour l'extension GNOME Shell), l'icône de la barre du haut et la fenêtre.
//!
//! Le service D-Bus démarre avant toute interface : une seconde instance s'arrête
//! avec un message sans jamais ouvrir d'affichage.

use std::process::ExitCode;

use engine::Engine;
use tauri::Wry;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Manager, WindowEvent};
use ultraclipboard::{BUS_NAME, config_path, start_app};

const MAIN_WINDOW: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_NOTIFICATIONS: &str = "notifications";
const MENU_QUIT: &str = "quit";

fn main() -> ExitCode {
    // Un seul runtime, partagé par le service D-Bus, le moteur et Tauri.
    let runtime = tokio::runtime::Runtime::new().expect("runtime tokio");
    let connect = || zbus::connection::Builder::session().build();
    // Gardée jusqu'à la fin : avec elle, l'envoi des notifications et le suivi de la config.
    let started = match runtime.block_on(start_app(config_path(), connect)) {
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
        .manage(started.conn.clone())
        .manage(started.engine.clone())
        .setup(|app| {
            add_tray(app)?;
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

/// Icône AppIndicator. Sous Linux le clic sur l'icône n'émet rien : tout passe par le menu.
fn add_tray(app: &App) -> tauri::Result<()> {
    let muted = app.state::<Engine>().notifications_muted();
    let notifications = MenuItem::with_id(
        app,
        MENU_NOTIFICATIONS,
        notifications_label(muted),
        true,
        None::<&str>,
    )?;
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, MENU_OPEN, "Ouvrir", true, None::<&str>)?,
            &notifications,
            &MenuItem::with_id(app, MENU_QUIT, "Quitter", true, None::<&str>)?,
        ],
    )?;
    let mut tray = TrayIconBuilder::new()
        .tooltip("UltraClipboard")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            MENU_OPEN => show_window(app),
            MENU_NOTIFICATIONS => toggle_notifications(app, &notifications),
            MENU_QUIT => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Coupe ou rétablit les notifications jusqu'au redémarrage.
fn toggle_notifications(app: &AppHandle, item: &MenuItem<Wry>) {
    let engine = app.state::<Engine>();
    let muted = !engine.notifications_muted();
    engine.set_notifications_muted(muted);
    let _ = item.set_text(notifications_label(muted));
}

/// Libellé de l'entrée de menu : l'action qu'elle fait.
fn notifications_label(muted: bool) -> &'static str {
    if muted {
        "Rétablir les notifications"
    } else {
        "Couper les notifications"
    }
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
