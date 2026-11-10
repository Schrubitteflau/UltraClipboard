//! L'appli lancée en seconde instance, sur un bus privé et sans affichage :
//! elle lit sa config puis s'arrête avant toute fenêtre.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use engine::{Config, Engine};
use ultraclipboard::serve;

/// Un `dbus-daemon` privé, arrêté à la fin du test. Ne touche jamais au bus de la session.
struct PrivateBus {
    daemon: Child,
    address: String,
}

impl PrivateBus {
    fn start() -> Self {
        // Socket sous le dossier temporaire de cargo, pas dans /tmp : un démon
        // tué ne le retire pas.
        let sockets = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("dbus");
        std::fs::create_dir_all(&sockets).unwrap();
        let mut daemon = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .arg(format!("--address=unix:tmpdir={}", sockets.display()))
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
/// d'affichage.
#[tokio::test]
async fn the_app_stops_with_a_message_when_already_running() {
    let bus = PrivateBus::start();
    let _first = first_instance(&bus).await;
    let home = scratch("already-running");

    let (status, stderr) = second_instance(&bus, &home);

    assert!(!status.success());
    assert!(stderr.contains("déjà lancé"), "stderr : {stderr}");
    for trace in ["Gtk", "tauri", "panicked"] {
        assert!(
            !stderr.contains(trace),
            "l'interface a démarré ; stderr : {stderr}"
        );
    }
}

#[tokio::test]
async fn the_app_takes_its_config_under_xdg_config_home() {
    let bus = PrivateBus::start();
    let _first = first_instance(&bus).await;
    let home = scratch("xdg-config-home");

    // Une seconde instance lit sa config avant de s'arrêter : elle crée l'exemple ici.
    second_instance(&bus, &home);

    let created = home
        .join("config")
        .join("ultraclipboard")
        .join("config.yaml");
    assert_eq!(
        std::fs::read_to_string(&created).unwrap(),
        engine::SAMPLE_CONFIG
    );
}

async fn first_instance(bus: &PrivateBus) -> zbus::Connection {
    let conn = zbus::connection::Builder::address(bus.address.as_str())
        .build()
        .await
        .unwrap();
    serve(&conn, Engine::new(Config::default())).await.unwrap();
    conn
}

/// Dossier vide propre au test, dans le dossier temporaire de cargo.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("desktop-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Lance le binaire sur `bus`, coupé de tout affichage réel : pas de `DISPLAY` ni
/// `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR` vide (sinon GTK retombe sur `wayland-0`,
/// le compositeur de l'utilisateur) et un backend broadway que personne n'écoute.
/// Config, données et cache sous `home`. Échoue si l'appli ne s'arrête pas d'elle-même.
fn second_instance(bus: &PrivateBus, home: &Path) -> (ExitStatus, String) {
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
    let status =
        status.unwrap_or_else(|| panic!("l'appli ne s'est pas arrêtée ; stderr : {stderr}"));
    (status, stderr)
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
