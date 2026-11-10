//! `upper` et `lower` : changent la casse du texte entier (Unicode).

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

pub(crate) struct Lower;

impl Transformation for Lower {
    fn name(&self) -> &str {
        "lower"
    }

    fn apply<'a>(&'a self, input: &'a str) -> StepFuture<'a> {
        Box::pin(async move { Ok(input.to_lowercase()) })
    }
}
