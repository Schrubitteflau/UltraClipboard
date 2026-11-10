//! `trim` : retire les espaces (au sens Unicode) en début et fin de texte.

use super::{StepFuture, Transformation};

pub(crate) struct Trim;

impl Transformation for Trim {
    fn name(&self) -> &str {
        "trim"
    }

    fn apply<'a>(&'a self, input: &'a str) -> StepFuture<'a> {
        Box::pin(async move { Ok(input.trim().to_owned()) })
    }
}
