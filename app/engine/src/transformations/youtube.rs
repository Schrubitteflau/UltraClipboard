//! `youtube` : lien Markdown d'une vidéo, via le service oEmbed public de
//! YouTube (sans clé d'API).
//!
//! Le texte copié (sans les espaces autour) est l'URL de la vidéo ; oEmbed rend
//! son titre, sa chaîne et sa miniature, que le gabarit met en forme.

use std::time::Duration;

use serde::Deserialize;

use super::{BuildError, Endpoints, StepError, StepFuture, Transformation};
use crate::config::yaml::Value;

/// Service oEmbed de YouTube.
pub(super) const OEMBED: &str = "https://www.youtube.com/oembed";

const DEFAULT_TEMPLATE: &str = "[{title}]({url})";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Champs utilisables dans le gabarit.
const FIELDS: [&str; 5] = ["title", "url", "author_name", "author_url", "thumbnail_url"];

pub(crate) struct Youtube {
    template: Vec<Piece>,
    timeout: Duration,
    oembed: String,
}

/// Morceau du gabarit : texte tel quel, ou champ à remplacer (index dans `FIELDS`).
enum Piece {
    Text(String),
    Field(usize),
}

pub(super) fn build(
    params: &Value,
    timeout: Option<Duration>,
    endpoints: &Endpoints,
) -> Result<Youtube, BuildError> {
    let mut template = DEFAULT_TEMPLATE;
    match params {
        Value::Null => {}
        Value::Map(entries) => {
            for (key, value) in entries {
                match (key.as_str(), value) {
                    ("template", Value::Str(text)) => template = text,
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
        }
        other => {
            return Err(BuildError::params(
                "",
                format!(
                    "attendu un dictionnaire (template), trouvé {}",
                    other.kind()
                ),
            ));
        }
    }
    Ok(Youtube {
        template: parse_template(template)?,
        timeout: timeout.unwrap_or(DEFAULT_TIMEOUT),
        oembed: endpoints.oembed.clone(),
    })
}

/// Découpe le gabarit.
///
/// `{nom}`, où `nom` n'a que des lettres ASCII, des chiffres et `_`, doit être
/// l'un des `FIELDS`, à la casse près (`{Title}` est refusé) : un nom inconnu
/// est une faute de frappe probable et la config est refusée. Toute autre
/// accolade reste telle quelle, y compris `{ title }` ou `{tïtle}`.
fn parse_template(template: &str) -> Result<Vec<Piece>, BuildError> {
    let mut pieces = Vec::new();
    let mut text = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        text.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let name = after.find('}').map(|close| &after[..close]);
        match name {
            Some(name) if is_placeholder(name) => {
                let index = FIELDS
                    .iter()
                    .position(|field| *field == name)
                    .ok_or_else(|| {
                        BuildError::params(
                            "template",
                            format!(
                                "champ inconnu {{{name}}} (attendu {})",
                                FIELDS.map(|f| format!("{{{f}}}")).join(", ")
                            ),
                        )
                    })?;
                if !text.is_empty() {
                    pieces.push(Piece::Text(std::mem::take(&mut text)));
                }
                pieces.push(Piece::Field(index));
                rest = &after[name.len() + 1..];
            }
            _ => {
                text.push('{');
                rest = after;
            }
        }
    }
    text.push_str(rest);
    if !text.is_empty() {
        pieces.push(Piece::Text(text));
    }
    Ok(pieces)
}

fn is_placeholder(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Taille maximale acceptée pour la réponse d'oEmbed (une vraie fait moins de 1 Kio).
const MAX_BODY: usize = 64 * 1024;

/// Réponse d'oEmbed. Seul le titre est exigé ; un autre champ absent vaut un
/// texte vide.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Video {
    title: Option<String>,
    author_name: String,
    author_url: String,
    thumbnail_url: String,
}

impl Youtube {
    async fn fetch(&self, url: &str) -> Result<Video, StepError> {
        let request =
            reqwest::Url::parse_with_params(&self.oembed, [("url", url), ("format", "json")])
                .map_err(|err| failure(format!("adresse oEmbed invalide : {err}")))?;
        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|err| failure(format!("client HTTP indisponible : {err}")))?;
        let mut response = client
            .get(request)
            .send()
            .await
            .map_err(|err| self.unreachable(err))?;
        let status = response.status();
        if status != reqwest::StatusCode::OK {
            return Err(failure(refused(status)));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|err| self.cut(err))? {
            if body.len() + chunk.len() > MAX_BODY {
                return Err(failure(format!(
                    "réponse oEmbed trop grosse (plus de {} Kio)",
                    MAX_BODY / 1024
                )));
            }
            body.extend_from_slice(&chunk);
        }
        let video: Video = serde_json::from_slice(&body)
            .map_err(|err| failure(format!("réponse oEmbed illisible : {err}")))?;
        match &video.title {
            Some(title) if !title.is_empty() => Ok(video),
            _ => Err(failure("réponse oEmbed sans titre".into())),
        }
    }

    /// Échec avant toute réponse.
    fn unreachable(&self, err: reqwest::Error) -> StepError {
        if err.is_timeout() {
            self.timed_out()
        } else {
            failure(format!("oEmbed injoignable : {err}"))
        }
    }

    /// Échec pendant la lecture de la réponse.
    fn cut(&self, err: reqwest::Error) -> StepError {
        if err.is_timeout() {
            self.timed_out()
        } else {
            failure(format!("réponse oEmbed interrompue : {err}"))
        }
    }

    fn timed_out(&self) -> StepError {
        failure(format!(
            "pas de réponse d'oEmbed en {} s",
            self.timeout.as_secs_f64()
        ))
    }

    fn render(&self, url: &str, video: &Video) -> String {
        let title = escape(video.title.as_deref().unwrap_or_default());
        let author_name = escape(&video.author_name);
        let values = [
            title.as_str(),
            url,
            &author_name,
            &video.author_url,
            &video.thumbnail_url,
        ];
        self.template
            .iter()
            .map(|piece| match piece {
                Piece::Text(text) => text.as_str(),
                Piece::Field(index) => values[*index],
            })
            .collect()
    }
}

/// Message d'échec pour une réponse autre que 200.
fn refused(status: reqwest::StatusCode) -> String {
    match status.as_u16() {
        400 | 401 | 403 | 404 => format!(
            "vidéo introuvable ou non intégrable (oEmbed : HTTP {status}) : \
             URL erronée, vidéo privée ou supprimée ?"
        ),
        429 => format!("oEmbed limite les requêtes (HTTP {status}) : réessayer plus tard"),
        500..=599 => format!("erreur du service oEmbed (HTTP {status})"),
        _ => format!("réponse oEmbed inattendue (HTTP {status})"),
    }
}

/// Échappe `\`, `[` et `]` pour qu'un titre ou un nom de chaîne ne casse pas
/// le texte d'un lien Markdown.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '[' | ']') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

fn failure(message: String) -> StepError {
    StepError {
        message,
        stderr: String::new(),
    }
}

impl Transformation for Youtube {
    fn name(&self) -> &str {
        "youtube"
    }

    fn apply<'a>(&'a self, input: &'a str) -> StepFuture<'a> {
        Box::pin(async move {
            let url = input.trim();
            let video = self.fetch(url).await?;
            Ok(self.render(url, &video))
        })
    }
}
