//! Moteur d'UltraClipboard : décide quoi écrire dans le presse-papier après une copie.
//!
//! Étape 1 (tranche verticale) : une seule Transformation codée en dur, `upper`.

/// Réagit aux copies et produit le texte à remettre dans le presse-papier.
#[derive(Debug, Default)]
pub struct Engine {
    /// Dernier texte écrit par UltraClipboard ; sa copie en retour ne doit pas être retraitée.
    last_output: Option<String>,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appelé à chaque copie. Renvoie le texte à écrire dans le presse-papier,
    /// ou `None` s'il ne faut rien toucher.
    pub fn on_copy(&mut self, text: &str) -> Option<String> {
        if self.last_output.take().is_some_and(|last| last == text) {
            return None;
        }
        let output = text.to_uppercase();
        if output == text {
            return None;
        }
        self.last_output = Some(output.clone());
        Some(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms_a_copy() {
        let mut engine = Engine::new();
        assert_eq!(engine.on_copy("hello"), Some("HELLO".to_string()));
    }

    #[test]
    fn ignores_its_own_output_once() {
        let mut engine = Engine::new();
        let out = engine.on_copy("hello").unwrap();
        // L'écriture du résultat déclenche une nouvelle « copie » : on l'ignore.
        assert_eq!(engine.on_copy(&out), None);
    }

    #[test]
    fn leaves_clipboard_alone_when_nothing_changes() {
        let mut engine = Engine::new();
        assert_eq!(engine.on_copy("ALREADY"), None);
    }

    #[test]
    fn a_new_copy_after_our_output_is_transformed() {
        let mut engine = Engine::new();
        let out = engine.on_copy("a").unwrap();
        engine.on_copy(&out);
        assert_eq!(engine.on_copy("b"), Some("B".to_string()));
    }
}
