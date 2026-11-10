//! Arbre YAML générique : la config est d'abord lue telle quelle, puis validée
//! champ par champ, ce qui permet de dire précisément où est l'erreur et laisse
//! chaque Transformation lire ses propres paramètres.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

/// Mots que le parseur lit comme des booléens quand ils sont sans guillemets.
const BOOLEAN_WORDS: &str = "on, off, yes, no, y, n, true, false";

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(String),
    Seq(Vec<Value>),
    /// Clés dans l'ordre du fichier.
    Map(Vec<(String, Value)>),
}

impl Value {
    /// Nom du type, pour les messages d'erreur.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Value::Null => "rien",
            Value::Bool(_) => "un booléen",
            Value::Int(_) | Value::Float(_) => "un nombre",
            Value::Str(_) => "un texte",
            Value::Seq(_) => "une liste",
            Value::Map(_) => "un dictionnaire",
        }
    }

    /// Comme [`Value::kind`], là où un texte est attendu : un booléen y vient
    /// presque toujours d'un mot non entouré de guillemets.
    pub(crate) fn kind_where_text_expected(&self) -> String {
        match self {
            Value::Bool(_) => format!(
                "{} ({BOOLEAN_WORDS} : mettre entre guillemets)",
                self.kind()
            ),
            other => other.kind().to_owned(),
        }
    }

    /// La valeur lue, pour les messages d'erreur : `1.0`, `« abc »`, ou son type.
    pub(crate) fn describe(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Float(x) => format!("{x:?}"),
            Value::Str(s) => format!("« {s} »"),
            other => other.kind().to_owned(),
        }
    }

    /// La valeur sur une ligne, à la manière du YAML en ligne : `"abc"`,
    /// `["a", 1]`, `clé: valeur, autre: 2` (accolades pour un dictionnaire imbriqué).
    pub(crate) fn inline(&self) -> String {
        match self {
            Value::Map(entries) => entries
                .iter()
                .map(|(key, value)| format!("{key}: {}", value.inline_nested()))
                .collect::<Vec<_>>()
                .join(", "),
            other => other.inline_nested(),
        }
    }

    fn inline_nested(&self) -> String {
        match self {
            Value::Null => "null".to_owned(),
            Value::Bool(b) => b.to_string(),
            Value::Int(n) => n.to_string(),
            Value::Float(x) => format!("{x:?}"),
            Value::Str(s) => format!("\"{}\"", escape_invisible(s)),
            Value::Seq(items) => format!(
                "[{}]",
                items
                    .iter()
                    .map(Value::inline_nested)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::Map(_) => format!("{{{}}}", self.inline()),
        }
    }
}

/// Le texte tel qu'écrit, sauf les caractères de contrôle et de direction
/// (bidi), qui changeraient la ligne affichée sans s'y voir.
fn escape_invisible(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() || is_bidi_or_line_break(c) => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Marques et contrôles de direction (bidi), séparateurs de ligne et de paragraphe.
fn is_bidi_or_line_break(c: char) -> bool {
    matches!(
        c,
        '\u{61c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("une valeur YAML")
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        Value::deserialize(d)
    }

    fn visit_bool<E>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Int(v.into()))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Int(v.into()))
    }

    fn visit_i128<E>(self, v: i128) -> Result<Value, E> {
        Ok(Value::Int(v))
    }

    fn visit_u128<E: de::Error>(self, v: u128) -> Result<Value, E> {
        i128::try_from(v)
            .map(Value::Int)
            .map_err(|_| E::custom("nombre trop grand"))
    }

    fn visit_f64<E>(self, v: f64) -> Result<Value, E> {
        Ok(Value::Float(v))
    }

    fn visit_str<E>(self, v: &str) -> Result<Value, E> {
        Ok(Value::Str(v.to_owned()))
    }

    fn visit_string<E>(self, v: String) -> Result<Value, E> {
        Ok(Value::Str(v))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Value::Seq(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut entries = Vec::new();
        while let Some((key, value)) = map.next_entry::<Value, Value>()? {
            let key = match key {
                Value::Str(s) => s,
                Value::Int(n) => n.to_string(),
                Value::Bool(b) => {
                    return Err(de::Error::custom(format!(
                        "clé lue comme le booléen {b} ({BOOLEAN_WORDS}) : la mettre entre guillemets"
                    )));
                }
                other => {
                    return Err(de::Error::custom(format!(
                        "clé invalide : {} au lieu d'un texte",
                        other.kind()
                    )));
                }
            };
            entries.push((key, value));
        }
        Ok(Value::Map(entries))
    }
}
