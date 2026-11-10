//! Appli UltraClipboard : expose le moteur sur le bus D-Bus de session.

use engine::Engine;
use ultraclipboard::{BUS_NAME, serve};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = zbus::connection::Builder::session().build().await?;
    if let Err(err) = serve(&conn, Engine::new()).await {
        eprintln!("ultraclipboard: {err}");
        std::process::exit(1);
    }

    eprintln!("ultraclipboard: prêt sur {BUS_NAME}");
    tokio::signal::ctrl_c().await?;
    Ok(())
}
