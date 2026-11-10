//! Une seule instance de l'appli : sur un bus privé, sans affichage.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use engine::Engine;
use ultraclipboard::serve;

/// Un `dbus-daemon` privé, arrêté à la fin du test. Ne touche jamais au bus de la session.
struct PrivateBus {
    daemon: Child,
    address: String,
}

impl PrivateBus {
    fn start() -> Self {
        let mut daemon = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon introuvable");
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        Self {
            daemon,
            address: address.trim().to_owned(),
        }
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

/// Le nom est vérifié avant toute fenêtre : l'appli s'arrête sans jamais ouvrir
/// d'affichage. L'enfant est coupé de tout affichage réel : pas de `DISPLAY` ni
/// `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR` vide (sinon GTK retombe sur `wayland-0`,
/// le compositeur de l'utilisateur) et un backend broadway que personne n'écoute.
#[tokio::test]
async fn the_app_stops_with_a_message_when_already_running() {
    let bus = PrivateBus::start();
    let first = zbus::connection::Builder::address(bus.address.as_str())
        .build()
        .await
        .unwrap();
    serve(&first, Engine::new()).await.unwrap();

    let home = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("single-instance-{}", std::process::id()));
    let runtime_dir = home.join("runtime");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ultraclipboard"))
        .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env("XDG_RUNTIME_DIR", &runtime_dir)
        .env("GDK_BACKEND", "broadway")
        .env("BROADWAY_DISPLAY", ":99")
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let status = wait_at_most(&mut child, Duration::from_secs(10));
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    let _ = std::fs::remove_dir_all(&home);

    let status =
        status.unwrap_or_else(|| panic!("l'appli ne s'est pas arrêtée ; stderr : {stderr}"));
    assert!(!status.success());
    assert!(stderr.contains("déjà lancé"), "stderr : {stderr}");
    for trace in ["Gtk", "tauri", "panicked"] {
        assert!(
            !stderr.contains(trace),
            "l'interface a démarré ; stderr : {stderr}"
        );
    }
}

/// Attend la fin de `child` ; passé le délai, le tue et rend `None`.
fn wait_at_most(child: &mut Child, limit: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}
