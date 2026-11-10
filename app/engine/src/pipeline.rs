//! Pipeline : suite ordonnée de Transformations, tout ou rien.

use crate::transformations::Transformation;
use crate::transformations::command::STDERR_CUT;

/// Échec d'une étape : le Pipeline s'arrête et ne produit rien.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// Nom de la Transformation qui a échoué.
    pub step: String,
    pub message: String,
    /// Sortie d'erreur de l'étape, vide si elle n'en a pas.
    pub stderr: String,
}

impl Failure {
    /// Cause probable : la dernière ligne non vide de la sortie d'erreur (celle
    /// gardée quand elle a été coupée) ; `None` sans sortie d'erreur.
    pub fn cause(&self) -> Option<&str> {
        self.stderr
            .lines()
            .rev()
            .map(str::trim_end)
            .find(|line| !line.is_empty() && !line.starts_with(STDERR_CUT))
    }
}

/// Suite ordonnée de Transformations.
pub(crate) struct Pipeline {
    steps: Vec<Box<dyn Transformation>>,
}

impl Pipeline {
    pub(crate) fn new(steps: Vec<Box<dyn Transformation>>) -> Self {
        Self { steps }
    }

    /// Applique les étapes l'une après l'autre ; la première qui échoue arrête tout.
    pub(crate) async fn run(&self, input: &str) -> Result<String, Failure> {
        let mut text = input.to_owned();
        for step in &self.steps {
            text = step.apply(&text).await.map_err(|err| Failure {
                step: step.name().to_owned(),
                message: err.message,
                stderr: err.stderr,
            })?;
        }
        Ok(text)
    }
}
