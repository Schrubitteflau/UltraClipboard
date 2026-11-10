//! Transformations : chaque fichier de ce dossier en définit une.

mod upper;

use std::future::Future;
use std::pin::Pin;

pub(crate) use upper::Upper;

/// Futur rendu par [`Transformation::apply`].
///
/// Seam de test temporaire, public seulement pour que les tests branchent une
/// étape lente ; à retirer quand une vraie étape `command` (`sleep`) pourra jouer ce rôle.
pub type StepFuture<'a> = Pin<Box<dyn Future<Output = Result<String, StepError>> + Send + 'a>>;

/// Opération qui prend un texte et rend un texte.
///
/// Seam de test temporaire, public seulement pour que les tests branchent une
/// étape lente ; à retirer quand une vraie étape `command` (`sleep`) pourra jouer ce rôle.
pub trait Transformation: Send + Sync {
    /// Nom tel qu'il apparaît dans la config et dans les échecs.
    fn name(&self) -> &str;

    fn apply<'a>(&'a self, input: &'a str) -> StepFuture<'a>;
}

/// Pourquoi une étape a échoué.
///
/// Seam de test temporaire, public seulement pour que les tests branchent une
/// étape lente ; à retirer quand une vraie étape `command` (`sleep`) pourra jouer ce rôle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StepError {
    pub message: String,
    /// Sortie d'erreur de l'étape, vide si elle n'en a pas.
    pub stderr: String,
}
