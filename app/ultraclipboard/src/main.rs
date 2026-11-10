//! Appli UltraClipboard : expose le moteur sur le bus D-Bus de session.

use engine::{Config, Engine};
use ultraclipboard::{BUS_NAME, config_path, serve, startup_config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = match config_path() {
        Some(path) => startup_config(&path),
        None => {
            eprintln!("ultraclipboard: ni XDG_CONFIG_HOME ni HOME, aucune Règle active");
            Config::default()
        }
    };
    let conn = zbus::connection::Builder::session().build().await?;
    if let Err(err) = serve(&conn, Engine::new(config)).await {
        eprintln!("ultraclipboard: {err}");
        std::process::exit(1);
    }

    eprintln!("ultraclipboard: prêt sur {BUS_NAME}");
    tokio::signal::ctrl_c().await?;
    Ok(())
}
