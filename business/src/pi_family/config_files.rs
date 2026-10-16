//! Reading JSON/JSONC/YAML configuration files into redacted [`ConfigSection`]s.
//!
//! Every read goes through [`SafeReader`] with the default 2 MiB cap. Parse failures are reported
//! by location only (`Invalid YAML at line 3, column 5`), never with file content.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::BusinessError;
use crate::model::{ConfigField, ConfigFormat, ConfigLayer, ConfigSection, ConfigSource, ConfigValue};
use crate::security::{Redactor, SafeReader};

/// A configuration file after reading and parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum Document {
    /// The file does not exist.
    Missing,
    /// Parsed content (YAML is converted to JSON values, keys in source order).
    Parsed(Value),
    /// The file exists but could not be read or parsed; secret-free summary.
    Invalid(String),
}

impl Document {
    /// The parsed value, if any.
    pub fn value(&self) -> Option<&Value> {
        match self {
            Document::Parsed(value) => Some(value),
            _ => None,
        }
    }

    /// The value at `key` of a parsed top-level object.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.value()?.get(key)
    }
}

/// Reads and parses `path` as `format` (JSON unless `format` is YAML or JSONC).
pub fn read_document(path: &Path, format: ConfigFormat) -> Document {
    match SafeReader::read_to_string(path, SafeReader::DEFAULT_MAX_BYTES) {
        Ok(text) => match parse_text(&text, format) {
            Ok(value) => Document::Parsed(value),
            Err(summary) => Document::Invalid(summary),
        },
        Err(BusinessError::NotFound(_)) => Document::Missing,
        Err(error) => Document::Invalid(error.to_string()),
    }
}

/// Reads a JSON file.
pub fn read_json(path: &Path) -> Document {
    read_document(path, ConfigFormat::Json)
}

/// Parses `text`; the error is a location-only summary.
pub fn parse_text(text: &str, format: ConfigFormat) -> Result<Value, String> {
    if format == ConfigFormat::Jsonc {
        return super::jsonc::parse(text);
    }

    if format != ConfigFormat::Yaml {
        return serde_json::from_str(text)
            .map_err(|error| format!("Invalid JSON at line {}, column {}", error.line(), error.column()));
    }

    if text.trim().is_empty() {
        return Ok(Value::Null);
    }

    serde_norway::from_str(text).map_err(|error| match error.location() {
        Some(location) => format!("Invalid YAML at line {}, column {}", location.line(), location.column()),
        None => "Invalid YAML".to_owned(),
    })
}

/// The first of `candidates` that exists, else the first one (where the file would be created).
pub fn first_existing(candidates: &[PathBuf]) -> PathBuf {
    candidates
        .iter()
        .find(|path| path.is_file())
        .or_else(|| candidates.first())
        .cloned()
        .unwrap_or_default()
}

/// A [`ConfigSource`] for `path`; `exists` is checked on disk.
pub fn source(path: &Path, layer: ConfigLayer, profile: Option<&str>, format: ConfigFormat) -> ConfigSource {
    ConfigSource {
        path: path.to_string_lossy().into_owned(),
        layer,
        profile: profile.map(str::to_owned),
        format,
        exists: path.is_file(),
    }
}

/// Fixed text of a section.
#[derive(Debug, Clone, Copy)]
pub struct SectionText<'a> {
    /// Stable identifier within the view.
    pub id: &'a str,
    /// Display title.
    pub title: &'a str,
    /// What the file controls.
    pub description: &'a str,
}

/// Reads `source.path` and renders it as a section; a missing file yields an empty section.
pub fn section(text: SectionText<'_>, source: ConfigSource) -> ConfigSection {
    let document = read_document(Path::new(&source.path), source.format);
    section_from(text, source, &document)
}

/// Like [`section`] but `None` when the file does not exist.
pub fn optional_section(text: SectionText<'_>, source: ConfigSource) -> Option<ConfigSection> {
    if !source.exists {
        return None;
    }

    Some(section(text, source))
}

/// Renders an already-read document.
pub fn section_from(text: SectionText<'_>, source: ConfigSource, document: &Document) -> ConfigSection {
    let (entries, error) = match document {
        Document::Missing => (Vec::new(), None),
        Document::Parsed(value) => match top_level_fields(value) {
            Ok(entries) => (entries, None),
            Err(error) => (Vec::new(), Some(error)),
        },
        Document::Invalid(summary) => (Vec::new(), Some(summary.clone())),
    };

    ConfigSection {
        id: text.id.to_owned(),
        title: text.title.to_owned(),
        description: Some(text.description.to_owned()),
        source,
        entries,
        error,
    }
}

/// Redacted top-level fields of an object; `null` (an empty YAML file) has none.
fn top_level_fields(value: &Value) -> Result<Vec<ConfigField>, String> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Object(_) => match Redactor.redact(value) {
            ConfigValue::Map(fields) => Ok(fields),
            _ => Ok(Vec::new()),
        },
        _ => Err("Expected a key/value object at the top level".to_owned()),
    }
}

/// The strings of a JSON array (other items are skipped); empty when `value` is not an array.
pub fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

/// A non-empty, trimmed string field.
pub fn text_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}
