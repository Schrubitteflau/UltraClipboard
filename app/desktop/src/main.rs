//! Appli UltraClipboard : un seul processus Tauri qui héberge le moteur, le service
//! D-Bus (pour l'extension GNOME Shell), l'icône de la barre du haut et la fenêtre.
//!
//! Le service D-Bus démarre avant toute interface : une seconde instance s'arrête
//! avec un message sans jamais ouvrir d'affichage.

use std::process::ExitCode;

use engine::Engine;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Manager, WindowEvent};
use ultraclipboard::{BUS_NAME, ServeError, serve};

const MAIN_WINDOW: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";

fn main() -> ExitCode {
    // Un seul runtime, partagé par le service D-Bus, le moteur et Tauri.
    let runtime = tokio::runtime::Runtime::new().expect("runtime tokio");
    let conn = match runtime.block_on(start_service()) {
        Ok(conn) => conn,
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

async fn start_service() -> Result<zbus::Connection, ServeError> {
    let conn = zbus::connection::Builder::session().build().await?;
    serve(&conn, Engine::new()).await?;
    Ok(conn)
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
