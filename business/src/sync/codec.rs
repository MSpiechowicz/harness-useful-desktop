//! Encoding and decoding of share bundles.
//!
//! A share string is `hud1:` + base64url (no padding) of the zlib-compressed compact JSON; a share
//! file is the pretty-printed JSON. zlib's Adler-32 only detects corruption: bundles are not signed,
//! so every received item is validated and reviewed by the user before anything runs.
//!
//! Decoding is defensive: every size is capped before the next stage allocates (pasted text,
//! base64 payload, decompressed JSON read through `take`, as a zip-bomb guard), serde_json keeps its
//! recursion limit, and structural problems (prefix, base64, zlib, size, JSON root, `format`,
//! `version`, section and item counts) reject the whole input with a [`DecodeError`] that never
//! echoes it. Problems inside one item only mark that item invalid (see [`super::schema`]).

use std::io::{Read, Write};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use serde_json::{Map, Value};

use super::schema::{
    Bundle, CODE_PREFIX, FORMAT, KNOWN_KINDS, MAX_CODE_BYTES, MAX_COMPRESSED_BYTES, MAX_ITEMS, MAX_ITEMS_PER_KIND,
    MAX_JSON_BYTES, MAX_PASTE_BYTES, MAX_SECTIONS, ParsedSection, VERSION, parse_section,
};
use super::validate;
use crate::error::{BusinessError, Result};
use crate::model::{HarnessId, SyncUnsupported};

/// Why an input was rejected as a whole. Messages describe the problem, never the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// Nothing but whitespace.
    #[error("the input is empty")]
    Empty,
    /// The text or file is over its size limit.
    #[error("the input is too large")]
    TooLarge,
    /// Neither JSON nor a `hud1:` string.
    #[error("the input is not a setup string (it must start with `hud1:`) or a setup file")]
    Prefix,
    /// The part after `hud1:` is not base64url.
    #[error("the setup string is damaged (not valid base64url)")]
    Base64,
    /// The compressed payload is over [`MAX_COMPRESSED_BYTES`].
    #[error("the setup string is too large")]
    CompressedTooLarge,
    /// zlib failed: corrupt, truncated (Adler-32 mismatch) or followed by extra bytes.
    #[error("the setup string is damaged or incomplete")]
    Decompress,
    /// The decompressed JSON is over [`MAX_JSON_BYTES`].
    #[error("the setup is too large once unpacked")]
    DecompressedTooLarge,
    /// Not valid JSON (including nesting deeper than serde_json's recursion limit).
    #[error("the setup is not valid JSON")]
    Json,
    /// The JSON root, or `harnesses`, or a known harness section, is not an object.
    #[error("the setup does not have the expected structure")]
    NotAnObject,
    /// `format` is not exactly [`FORMAT`].
    #[error("this is not a Harness Desktop setup")]
    Format,
    /// `version` is missing, not a positive integer.
    #[error("the setup has no valid version")]
    Version,
    /// `version` is newer than [`VERSION`].
    #[error("this setup was made by a newer version of the app; update the app to import it")]
    NewerVersion,
    /// More than [`MAX_SECTIONS`] harness sections.
    #[error("the setup has too many harness sections")]
    TooManySections,
    /// More than [`MAX_ITEMS_PER_KIND`] items in one array or [`MAX_ITEMS`] in total.
    #[error("the setup has too many items")]
    TooManyItems,
}

impl From<DecodeError> for BusinessError {
    fn from(error: DecodeError) -> Self {
        match error {
            DecodeError::NewerVersion => BusinessError::Unsupported(error.to_string()),
            _ => BusinessError::InvalidInput(error.to_string()),
        }
    }
}

/// A received bundle: the known harnesses parsed item by item, plus what this app does not know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedBundle {
    /// The app version that wrote it, when present and safe to show.
    pub app: Option<String>,
    /// Its export date, when present and safe to show.
    pub created_at: Option<String>,
    /// The known harnesses present in the bundle, in [`HarnessId::ALL`] order.
    pub sections: Vec<ParsedSection>,
    /// Unknown harness sections (`harness: None`) and unknown item kinds of known harnesses, in the
    /// bundle's order, with item counts only.
    pub unsupported: Vec<SyncUnsupported>,
}

impl DecodedBundle {
    /// How many items were refused across every section.
    pub fn invalid_count(&self) -> usize {
        self.sections.iter().map(ParsedSection::invalid_count).sum()
    }
}

/// The share string of `bundle`. Refused (`invalidInput`) when its JSON is over [`MAX_JSON_BYTES`]
/// or the string is over [`MAX_CODE_BYTES`]: such a setup must be saved as a file.
pub fn encode_code(bundle: &Bundle) -> Result<String> {
    let json =
        serde_json::to_vec(bundle).map_err(|error| BusinessError::Internal(format!("encoding a setup: {error}")))?;
    if json.len() > MAX_JSON_BYTES {
        return Err(too_large_for("a setup"));
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    let compressed = encoder
        .write_all(&json)
        .and_then(|()| encoder.finish())
        .map_err(|error| BusinessError::Internal(format!("compressing a setup: {error}")))?;

    let code = format!("{CODE_PREFIX}{}", URL_SAFE_NO_PAD.encode(compressed));
    if code.len() > MAX_CODE_BYTES {
        return Err(too_large_for("a share string; save it as a file instead"));
    }

    Ok(code)
}

/// The share file contents of `bundle` (pretty JSON). Refused (`invalidInput`) over
/// [`MAX_JSON_BYTES`], so every saved file can be imported again.
pub fn to_json_pretty(bundle: &Bundle) -> Result<String> {
    let json = serde_json::to_string_pretty(bundle)
        .map_err(|error| BusinessError::Internal(format!("encoding a setup: {error}")))?;

    if json.len() > MAX_JSON_BYTES {
        return Err(too_large_for("a setup file"));
    }

    Ok(json)
}

fn too_large_for(what: &str) -> BusinessError {
    BusinessError::InvalidInput(format!("the selected items are too large for {what}"))
}

/// Decodes pasted text (at most [`MAX_PASTE_BYTES`]): a `hud1:` string (surrounding and inner ASCII
/// whitespace ignored, prefix case-sensitive) or, when it starts with `{`, the JSON of a share file.
pub fn decode_input(text: &str) -> Result<DecodedBundle, DecodeError> {
    decode(text, MAX_PASTE_BYTES)
}

/// Decodes a share file's bytes (at most [`MAX_JSON_BYTES`], UTF-8, an optional BOM): its JSON, or
/// a `hud1:` string saved as text.
pub fn decode_file(bytes: &[u8]) -> Result<DecodedBundle, DecodeError> {
    if bytes.len() > MAX_JSON_BYTES {
        return Err(DecodeError::TooLarge);
    }

    let text = std::str::from_utf8(bytes).map_err(|_| DecodeError::Json)?;
    decode(text.trim_start_matches('\u{feff}'), MAX_JSON_BYTES)
}

fn decode(text: &str, max_bytes: usize) -> Result<DecodedBundle, DecodeError> {
    if text.len() > max_bytes {
        return Err(DecodeError::TooLarge);
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(DecodeError::Empty);
    }

    let value: Value = if trimmed.starts_with('{') {
        serde_json::from_str(trimmed).map_err(|_| DecodeError::Json)?
    } else {
        serde_json::from_slice(&unpack_code(trimmed)?).map_err(|_| DecodeError::Json)?
    };

    parse_root(value)
}

/// `hud1:` string -> decompressed JSON bytes.
fn unpack_code(text: &str) -> Result<Vec<u8>, DecodeError> {
    let compact: String = text
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect();
    let payload = compact.strip_prefix(CODE_PREFIX).ok_or(DecodeError::Prefix)?;

    // base64 holds 3 bytes per 4 characters: refuse before decoding anything larger.
    if payload.len() > MAX_COMPRESSED_BYTES.div_ceil(3) * 4 {
        return Err(DecodeError::CompressedTooLarge);
    }

    let compressed = URL_SAFE_NO_PAD.decode(payload).map_err(|_| DecodeError::Base64)?;
    if compressed.len() > MAX_COMPRESSED_BYTES {
        return Err(DecodeError::CompressedTooLarge);
    }

    let mut decoder = ZlibDecoder::new(compressed.as_slice());
    let mut json = Vec::new();
    let limit = MAX_JSON_BYTES as u64 + 1;
    (&mut decoder)
        .take(limit)
        .read_to_end(&mut json)
        .map_err(|_| DecodeError::Decompress)?;

    if json.len() > MAX_JSON_BYTES {
        return Err(DecodeError::DecompressedTooLarge);
    }

    if decoder.total_in() != compressed.len() as u64 {
        return Err(DecodeError::Decompress);
    }

    Ok(json)
}

fn parse_root(value: Value) -> Result<DecodedBundle, DecodeError> {
    let Value::Object(root) = value else {
        return Err(DecodeError::NotAnObject);
    };

    if root.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(DecodeError::Format);
    }

    match root.get("version").and_then(Value::as_u64) {
        Some(version) if version > u64::from(VERSION) => return Err(DecodeError::NewerVersion),
        Some(version) if version >= 1 => {}
        _ => return Err(DecodeError::Version),
    }

    let empty = Map::new();
    let harnesses = match root.get("harnesses") {
        None | Some(Value::Null) => &empty,
        Some(Value::Object(harnesses)) => harnesses,
        Some(_) => return Err(DecodeError::NotAnObject),
    };

    if harnesses.len() > MAX_SECTIONS {
        return Err(DecodeError::TooManySections);
    }

    let mut total = 0;
    let mut sections = Vec::new();
    let mut unsupported = Vec::new();

    for (key, section) in harnesses {
        let harness = serde_json::from_value::<HarnessId>(Value::String(key.clone())).ok();
        let section = match (harness, section) {
            (_, Value::Object(section)) => Some(section),
            (Some(_), _) => return Err(DecodeError::NotAnObject),
            (None, _) => None,
        };

        let counts = section.map(item_counts).transpose()?.unwrap_or_default();
        total += counts.iter().map(|(_, count)| count).sum::<usize>();

        match (harness, section) {
            (Some(harness), Some(section)) => {
                sections.push(parse_section(harness, section));
                unsupported.extend(unknown_kinds(harness, counts));
            }
            _ => unsupported.push(SyncUnsupported {
                harness: None,
                name: validate::section_label(key),
                item_count: count_u32(counts.iter().map(|(_, count)| count).sum()),
            }),
        }
    }

    if total > MAX_ITEMS {
        return Err(DecodeError::TooManyItems);
    }

    sections.sort_by_key(|section| HarnessId::ALL.iter().position(|id| *id == section.harness));

    Ok(DecodedBundle {
        app: info_field(&root, "app", |app| validate::version(Some(app))),
        created_at: info_field(&root, "createdAt", validate::date),
        sections,
        unsupported,
    })
}

/// The length of every array in a section, refusing an array over [`MAX_ITEMS_PER_KIND`].
fn item_counts(section: &Map<String, Value>) -> Result<Vec<(&str, usize)>, DecodeError> {
    let mut counts = Vec::new();

    for (key, value) in section {
        let Value::Array(items) = value else {
            continue;
        };

        if items.len() > MAX_ITEMS_PER_KIND {
            return Err(DecodeError::TooManyItems);
        }

        counts.push((key.as_str(), items.len()));
    }

    Ok(counts)
}

fn unknown_kinds(harness: HarnessId, counts: Vec<(&str, usize)>) -> impl Iterator<Item = SyncUnsupported> {
    counts
        .into_iter()
        .filter(|(key, _)| !KNOWN_KINDS.contains(key))
        .map(move |(key, count)| SyncUnsupported {
            harness: Some(harness),
            name: validate::section_label(key),
            item_count: count_u32(count),
        })
}

/// The text at `key` when `check` accepts it; anything else is dropped.
fn info_field(root: &Map<String, Value>, key: &str, check: impl Fn(&str) -> validate::Check) -> Option<String> {
    let value = root.get(key)?.as_str()?;
    check(value).is_ok().then(|| value.to_owned())
}

fn count_u32(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}
