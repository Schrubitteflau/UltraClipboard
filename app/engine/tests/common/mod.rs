//! Outils partagés des tests qui lancent de vrais processus.

#![allow(dead_code)] // chaque fichier de test n'utilise qu'une partie des outils

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Fichier de travail propre au test, sous le dossier temporaire de cargo ;
/// absent au départ.
pub fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("engine");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    path
}

/// Vrai tant que le processus `pid` existe et n'est pas un zombie en attente de
/// ramassage (un processus tué n'existe plus : seul son numéro reste un instant).
pub fn alive(pid: u32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        // Troisième champ, après le nom entre parenthèses : l'état.
        Ok(stat) => !stat
            .rsplit_once(") ")
            .is_some_and(|(_, rest)| rest.starts_with('Z')),
        Err(_) => false,
    }
}

/// Attend (2 s au plus) que le processus `pid` meure ; faux s'il vit encore.
pub async fn wait_dead(pid: u32) -> bool {
    for _ in 0..100 {
        if !alive(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

/// Numéros de processus, un par ligne.
pub fn pids(text: &str) -> Vec<u32> {
    text.lines().map(|line| line.parse().unwrap()).collect()
}

/// Attend (5 s au plus) que `count` numéros de processus soient écrits dans
/// `pidfile`, un par ligne, et les rend ; fait échouer le test sinon.
pub async fn pids_written_to(pidfile: &Path, count: usize) -> Vec<u32> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = std::fs::read_to_string(pidfile)
            && text.ends_with('\n')
            && text.lines().count() >= count
        {
            return pids(&text);
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{count} numéro(s) de processus attendu(s) dans {}",
            pidfile.display()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
