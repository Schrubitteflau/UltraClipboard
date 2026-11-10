//! `upper` : met le texte entier en majuscules (Unicode).

use super::{StepFuture, Transformation};

pub(crate) struct Upper;

impl Transformation for Upper {
    fn name(&self) -> &str {
        "upper"
    }

    fn apply<'a>(&'a self, input: &'a str) -> StepFuture<'a> {
        Box::pin(async move { Ok(input.to_uppercase()) })
    }
}
