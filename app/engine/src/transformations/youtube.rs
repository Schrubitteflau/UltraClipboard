//! `youtube` : lien Markdown d'une vidéo, via oEmbed.
//!
//! Provisoire : seuls les paramètres sont lus, pour que la config d'exemple
//! soit valide ; l'appel à oEmbed arrive avec le ticket #7. D'ici là, l'étape
//! échoue et le presse-papier n'est pas touché.

use std::time::Duration;

use super::{BuildError, StepError, StepFuture, Transformation};
use crate::config::yaml::Value;

pub(crate) struct Youtube;

pub(super) fn build(params: &Value, _timeout: Option<Duration>) -> Result<Youtube, BuildError> {
    match params {
        Value::Null => Ok(Youtube),
        Value::Map(entries) => {
            for (key, value) in entries {
                match (key.as_str(), value) {
                    ("template", Value::Str(_)) => {}
                    ("template", other) => {
                        return Err(BuildError::params(
                            "template",
                            format!(
                                "attendu un texte, trouvé {}",
                                other.kind_where_text_expected()
                            ),
                        ));
                    }
                    (other, _) => return Err(BuildError::params(other, "paramètre inconnu")),
                }
            }
            Ok(Youtube)
        }
        other => Err(BuildError::params(
            "",
            format!(
                "attendu un dictionnaire (template), trouvé {}",
                other.kind()
            ),
        )),
    }
}

impl Transformation for Youtube {
    fn name(&self) -> &str {
        "youtube"
    }

    fn apply<'a>(&'a self, _input: &'a str) -> StepFuture<'a> {
        Box::pin(async {
            Err(StepError {
                message: "pas encore disponible".into(),
                stderr: String::new(),
            })
        })
    }
}
