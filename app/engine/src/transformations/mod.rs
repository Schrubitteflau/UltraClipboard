//! Transformations : chaque fichier de ce dossier en définit une.
//!
//! [`build`] est le seul endroit qui relie un nom de la config à une
//! Transformation : en ajouter une, c'est un fichier et une ligne ici.

mod case;
mod trim;
mod youtube;

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::config::yaml::Value;

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

/// Pourquoi une étape de la config n'a pas pu être construite.
pub(crate) enum BuildError {
    /// Aucune Transformation ne porte ce nom.
    Unknown,
    /// Paramètres refusés ; `field` est relatif aux paramètres (vide : les
    /// paramètres dans leur ensemble).
    Params { field: String, reason: String },
}

impl BuildError {
    fn params(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Params {
            field: field.into(),
            reason: reason.into(),
        }
    }
}

/// Adresses des services en ligne appelés par les Transformations.
///
/// Réglées par l'appli, jamais par la config utilisateur : les tests y mettent
/// un faux service local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// Service oEmbed de YouTube (`youtube`).
    pub oembed: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            oembed: youtube::OEMBED.to_owned(),
        }
    }
}

/// Construit l'étape `name` à partir de ses paramètres (`Null` si l'étape est
/// écrite par son seul nom) et du `timeout` éventuel de l'étape.
pub(crate) fn build(
    name: &str,
    params: &Value,
    timeout: Option<Duration>,
    endpoints: &Endpoints,
) -> Result<Box<dyn Transformation>, BuildError> {
    match name {
        "trim" => no_params(params).map(|()| Box::new(trim::Trim) as _),
        "upper" => no_params(params).map(|()| Box::new(case::Upper) as _),
        "lower" => no_params(params).map(|()| Box::new(case::Lower) as _),
        "youtube" => youtube::build(params, timeout, endpoints).map(|t| Box::new(t) as _),
        _ => Err(BuildError::Unknown),
    }
}

fn no_params(params: &Value) -> Result<(), BuildError> {
    match params {
        Value::Null => Ok(()),
        _ => Err(BuildError::params("", "n'accepte pas de paramètre")),
    }
}
