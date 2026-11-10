//! Config utilisateur (`config.yaml`) : chargement et validation.
//!
//! Le texte YAML est d'abord lu comme un arbre générique ([`yaml::Value`]), puis
//! validé champ par champ : chaque erreur nomme le champ fautif. Les étapes des
//! Pipelines sont construites par [`transformations::build`], qui seul connaît
//! les Transformations et leurs paramètres.

pub(crate) mod yaml;

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use regex::Regex;

use crate::pipeline::Pipeline;
use crate::transformations::{self, BuildError, Transformation};
use yaml::Value;

/// Fichier d'exemple créé au premier lancement quand la config n'existe pas.
pub const SAMPLE_CONFIG: &str = include_str!("config/sample.yaml");

/// Config validée : les Règles, dans l'ordre du fichier, et leurs Pipelines.
#[derive(Default)]
pub struct Config {
    notifications: Notifications,
    rules: Vec<Rule>,
}

/// Niveau de notification choisi dans la config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Notifications {
    /// Succès et échecs.
    #[default]
    All,
    /// Échecs et config invalide seulement.
    Errors,
    Off,
}

/// Association d'un motif et d'un Pipeline.
pub struct Rule {
    pattern: Regex,
    pipeline_name: String,
    pipeline: Arc<Pipeline>,
}

impl Rule {
    /// Expression régulière telle qu'écrite dans la config.
    pub fn pattern(&self) -> &str {
        self.pattern.as_str()
    }

    /// Nom du Pipeline appliqué.
    pub fn pipeline(&self) -> &str {
        &self.pipeline_name
    }

    pub(crate) fn run(&self) -> Arc<Pipeline> {
        Arc::clone(&self.pipeline)
    }
}

/// Config refusée : quel champ, et pourquoi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    /// Chemin du champ fautif (`rules[0].match`, `pipelines.clean[1]`…) ; vide
    /// quand c'est le document entier (YAML illisible).
    pub field: String,
    pub reason: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.field.is_empty() {
            write!(f, "{}", self.reason)
        } else {
            write!(f, "{} : {}", self.field, self.reason)
        }
    }
}

impl std::error::Error for ConfigError {}

fn error(field: impl Into<String>, reason: impl Into<String>) -> ConfigError {
    ConfigError {
        field: field.into(),
        reason: reason.into(),
    }
}

impl Config {
    /// Lit et valide une config YAML.
    pub fn parse(yaml: &str) -> Result<Self, ConfigError> {
        let root: Value = serde_saphyr::from_str(yaml)
            .map_err(|err| error("", format!("YAML invalide : {err}")))?;
        let root = match root {
            Value::Map(entries) => entries,
            Value::Null => Vec::new(),
            other => {
                return Err(error(
                    "",
                    format!("attendu un dictionnaire, trouvé {}", other.kind()),
                ));
            }
        };

        check_version(field(&root, "config_version"))?;
        for (key, _) in &root {
            if !["config_version", "notifications", "pipelines", "rules"].contains(&key.as_str()) {
                return Err(error(key.as_str(), "champ inconnu"));
            }
        }
        let notifications = parse_notifications(field(&root, "notifications"))?;
        let pipelines = parse_pipelines(field(&root, "pipelines"))?;
        let rules = parse_rules(field(&root, "rules"), &pipelines)?;
        Ok(Self {
            notifications,
            rules,
        })
    }

    /// Règles, dans l'ordre du fichier.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn notifications(&self) -> Notifications {
        self.notifications
    }

    /// Première Règle dont le motif est trouvé dans le texte.
    pub(crate) fn rule_for(&self, text: &str) -> Option<&Rule> {
        self.rules.iter().find(|rule| rule.pattern.is_match(text))
    }

    /// Une seule Règle qui applique ce Pipeline à tout texte.
    pub(crate) fn match_all(name: &str, pipeline: Pipeline) -> Self {
        Self {
            notifications: Notifications::default(),
            rules: vec![Rule {
                pattern: Regex::new("").unwrap(),
                pipeline_name: name.to_owned(),
                pipeline: Arc::new(pipeline),
            }],
        }
    }
}

fn field<'a>(entries: &'a [(String, Value)], name: &str) -> Option<&'a Value> {
    entries
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
}

fn check_version(version: Option<&Value>) -> Result<(), ConfigError> {
    match version {
        Some(Value::Int(1)) => Ok(()),
        None | Some(Value::Null) => Err(error(
            "config_version",
            "obligatoire (écrire `config_version: 1`)",
        )),
        Some(Value::Int(n)) => Err(error(
            "config_version",
            format!("version inconnue : {n} (seule la version 1 existe)"),
        )),
        Some(other) => Err(error(
            "config_version",
            format!(
                "version inconnue : {} (attendu l'entier 1)",
                other.describe()
            ),
        )),
    }
}

fn parse_notifications(value: Option<&Value>) -> Result<Notifications, ConfigError> {
    match value {
        None | Some(Value::Null) => Ok(Notifications::default()),
        Some(Value::Str(s)) if s == "all" => Ok(Notifications::All),
        Some(Value::Str(s)) if s == "errors" => Ok(Notifications::Errors),
        // Le parseur lit `off` sans guillemets comme le booléen faux (YAML 1.1).
        Some(Value::Str(s)) if s == "off" => Ok(Notifications::Off),
        Some(Value::Bool(false)) => Ok(Notifications::Off),
        Some(Value::Str(s)) => Err(error(
            "notifications",
            format!("valeur inconnue « {s} » (attendu all, errors ou off)"),
        )),
        Some(other) => Err(error(
            "notifications",
            format!(
                "attendu all, errors ou off, trouvé {}",
                other.kind_where_text_expected()
            ),
        )),
    }
}

fn parse_pipelines(value: Option<&Value>) -> Result<HashMap<String, Arc<Pipeline>>, ConfigError> {
    let entries = match value {
        None | Some(Value::Null) => return Ok(HashMap::new()),
        Some(Value::Map(entries)) => entries,
        Some(other) => {
            return Err(error(
                "pipelines",
                format!(
                    "attendu un dictionnaire de Pipelines nommés, trouvé {}",
                    other.kind()
                ),
            ));
        }
    };
    let mut pipelines = HashMap::new();
    for (name, steps) in entries {
        let path = format!("pipelines.{name}");
        let steps = match steps {
            Value::Seq(steps) if !steps.is_empty() => steps,
            Value::Seq(_) | Value::Null => {
                return Err(error(path, "Pipeline vide (au moins une étape)"));
            }
            other => {
                return Err(error(
                    path,
                    format!("attendu une liste d'étapes, trouvé {}", other.kind()),
                ));
            }
        };
        let steps = steps
            .iter()
            .enumerate()
            .map(|(i, step)| parse_step(&format!("{path}[{i}]"), step))
            .collect::<Result<Vec<_>, _>>()?;
        pipelines.insert(name.clone(), Arc::new(Pipeline::new(steps)));
    }
    Ok(pipelines)
}

/// Une étape : `nom`, ou `nom: paramètres` avec un `timeout` facultatif à côté.
fn parse_step(path: &str, step: &Value) -> Result<Box<dyn Transformation>, ConfigError> {
    let (name, params, timeout) = match step {
        Value::Str(name) => (name.as_str(), &Value::Null, None),
        Value::Map(entries) => {
            let timeout = field(entries, "timeout")
                .map(|t| parse_timeout(&format!("{path}.timeout"), t))
                .transpose()?;
            let named: Vec<_> = entries.iter().filter(|(key, _)| key != "timeout").collect();
            match named.as_slice() {
                [(name, params)] => (name.as_str(), params, timeout),
                [] => return Err(error(path, "aucune Transformation nommée")),
                many => {
                    let names: Vec<_> = many.iter().map(|(key, _)| key.as_str()).collect();
                    return Err(error(
                        path,
                        format!(
                            "une étape nomme une seule Transformation, trouvé : {}",
                            names.join(", ")
                        ),
                    ));
                }
            }
        }
        other => {
            return Err(error(
                path,
                format!(
                    "attendu le nom d'une Transformation, trouvé {}",
                    other.kind_where_text_expected()
                ),
            ));
        }
    };
    transformations::build(name, params, timeout).map_err(|err| match err {
        BuildError::Unknown => error(path, format!("Transformation inconnue « {name} »")),
        BuildError::Params { field, reason } => {
            let mut at = format!("{path}.{name}");
            if !field.is_empty() {
                at = format!("{at}.{field}");
            }
            error(at, reason)
        }
    })
}

fn parse_timeout(path: &str, value: &Value) -> Result<Duration, ConfigError> {
    let seconds = match value {
        Value::Int(n) => *n as f64,
        Value::Float(x) => *x,
        other => {
            return Err(error(
                path,
                format!("attendu un nombre de secondes, trouvé {}", other.kind()),
            ));
        }
    };
    Duration::try_from_secs_f64(seconds)
        .ok()
        .filter(|d| !d.is_zero())
        .ok_or_else(|| error(path, "attendu un nombre de secondes positif"))
}

fn parse_rules(
    value: Option<&Value>,
    pipelines: &HashMap<String, Arc<Pipeline>>,
) -> Result<Vec<Rule>, ConfigError> {
    let items = match value {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Seq(items)) => items,
        Some(other) => {
            return Err(error(
                "rules",
                format!("attendu une liste de Règles, trouvé {}", other.kind()),
            ));
        }
    };
    items
        .iter()
        .enumerate()
        .map(|(i, item)| parse_rule(&format!("rules[{i}]"), item, pipelines))
        .collect()
}

fn parse_rule(
    path: &str,
    item: &Value,
    pipelines: &HashMap<String, Arc<Pipeline>>,
) -> Result<Rule, ConfigError> {
    let Value::Map(entries) = item else {
        return Err(error(
            path,
            format!(
                "attendu une Règle (match, pipeline), trouvé {}",
                item.kind()
            ),
        ));
    };
    for (key, _) in entries {
        if key != "match" && key != "pipeline" {
            return Err(error(format!("{path}.{key}"), "champ inconnu"));
        }
    }
    let pattern = required_str(path, entries, "match")?;
    let pattern = Regex::new(pattern).map_err(|err| {
        error(
            format!("{path}.match"),
            format!("expression régulière invalide : {err}"),
        )
    })?;
    let name = required_str(path, entries, "pipeline")?;
    let pipeline = pipelines.get(name).ok_or_else(|| {
        error(
            format!("{path}.pipeline"),
            format!("Pipeline inconnu « {name} »"),
        )
    })?;
    Ok(Rule {
        pattern,
        pipeline_name: name.to_owned(),
        pipeline: Arc::clone(pipeline),
    })
}

fn required_str<'a>(
    path: &str,
    entries: &'a [(String, Value)],
    name: &str,
) -> Result<&'a str, ConfigError> {
    match field(entries, name) {
        Some(Value::Str(s)) => Ok(s),
        None | Some(Value::Null) => Err(error(format!("{path}.{name}"), "obligatoire")),
        Some(other) => Err(error(
            format!("{path}.{name}"),
            format!(
                "attendu un texte, trouvé {}",
                other.kind_where_text_expected()
            ),
        )),
    }
}
