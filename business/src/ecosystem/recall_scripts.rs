//! The saved scripts side of memory recall: reading the scripts `harness-memory` found for a
//! message out of its `memory_search` reply, and the lines they get in the recall block.
//!
//! The server appends a block to a `memory_search` reply when scripts match the query:
//!
//! ```text
//! <saved-scripts>
//! - fetch-issues: Lists open issues as JSON (sh) → script_run name=fetch-issues
//! </saved-scripts>
//! ```
//!
//! A reply with `structuredContent` also carries the scripts as `scripts: [{name, description,
//! language}]`, which is preferred when it is there. Reading them out of the reply that is already
//! at hand costs no second call, so a search that finds no script (or a reply without the block)
//! simply recalls notes alone, and the scripts are matched by the server with the very keywords
//! the notes were.
//!
//! Script names and descriptions are agent-written data: they are cut to one short line each, and
//! `recall_text` frames them as data and removes every recall marker from them.

use serde_json::Value;

/// Most scripts read from one reply.
pub const RECALL_SCRIPTS_MAX: usize = 5;

/// First line of the block in a `memory_search` reply.
const REPLY_OPEN: &str = "<saved-scripts>";

/// Last line of the block in a `memory_search` reply (a reply without it ends the block at its end).
const REPLY_CLOSE: &str = "</saved-scripts>";

/// Prefix of the lines in the reply block.
const REPLY_BULLET: &str = "- ";

/// Between the description and the run hint in a reply line.
const REPLY_ARROW: &str = " → ";

/// Longest script name in bytes (`harness-memory` allows 64).
const NAME_MAX: usize = 64;

/// Longest description kept in a recall line, in bytes.
const DESCRIPTION_MAX: usize = 120;

/// Longest language name kept, in bytes.
const LANGUAGE_MAX: usize = 12;

/// A saved script a message matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallScript {
    /// The name `script_run` takes.
    pub name: String,
    /// One line of at most 120 bytes; may be empty.
    pub description: String,
    /// `sh`, `python` and the like; empty when the reply names none.
    pub language: String,
}

impl RecallScript {
    /// A script from raw `name`, `description` and `language`; `None` when the name is not one
    /// `harness-memory` allows (`[a-z0-9][a-z0-9_-]*`, at most 64 bytes).
    pub fn new(name: &str, description: &str, language: &str) -> Option<Self> {
        let name = name.trim();

        if !is_script_name(name) {
            return None;
        }

        let language: String = language.chars().filter(|c| c.is_ascii_alphanumeric()).collect();

        Some(Self {
            name: name.to_owned(),
            description: one_line(description, DESCRIPTION_MAX),
            language: one_line(&language, LANGUAGE_MAX),
        })
    }

    /// `name: description (language)`; parts that are empty are left out.
    pub fn summary(&self) -> String {
        let mut line = self.name.clone();

        if !self.description.is_empty() {
            line.push_str(": ");
            line.push_str(&self.description);
        }

        if !self.language.is_empty() {
            line.push_str(&format!(" ({})", self.language));
        }

        line
    }
}

/// The scripts in a `memory_search` result: from `structured` when it holds a `scripts` array,
/// otherwise from the `<saved-scripts>` block of `text`. At most [`RECALL_SCRIPTS_MAX`], in the
/// server's order, each name once; lines and objects that do not read as a script are skipped.
pub fn scripts_of(structured: Option<&Value>, text: &str) -> Vec<RecallScript> {
    let scripts = match structured
        .and_then(|value| value.get("scripts"))
        .and_then(Value::as_array)
    {
        Some(objects) => objects.iter().filter_map(from_object).collect(),
        None => recall_scripts(text),
    };

    distinct(scripts)
}

/// The scripts in the `<saved-scripts>` block of a `memory_search` reply text. The first
/// [`RECALL_SCRIPTS_MAX`] distinct names.
pub fn recall_scripts(reply: &str) -> Vec<RecallScript> {
    let mut lines = reply
        .lines()
        .map(str::trim_end)
        .skip_while(|line| line.trim() != REPLY_OPEN);
    lines.next();

    let scripts = lines
        .take_while(|line| line.trim() != REPLY_CLOSE)
        .filter_map(from_line)
        .collect();

    distinct(scripts)
}

fn distinct(scripts: Vec<RecallScript>) -> Vec<RecallScript> {
    let mut kept: Vec<RecallScript> = Vec::new();

    for script in scripts {
        if kept.len() >= RECALL_SCRIPTS_MAX {
            break;
        }

        if !kept.iter().any(|other| other.name == script.name) {
            kept.push(script);
        }
    }

    kept
}

fn from_object(object: &Value) -> Option<RecallScript> {
    let text = |key: &str| object.get(key).and_then(Value::as_str).unwrap_or_default();

    RecallScript::new(text("name"), text("description"), text("language"))
}

/// Whether `name` is a script name: `[a-z0-9][a-z0-9_-]*`, at most [`NAME_MAX`] bytes.
fn is_script_name(name: &str) -> bool {
    let mut chars = name.chars();
    let starts = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());

    starts
        && name.len() <= NAME_MAX
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// `- name: description (language) → script_run name=name`; the description, the language and the
/// run hint are optional.
fn from_line(line: &str) -> Option<RecallScript> {
    let rest = line.trim().strip_prefix(REPLY_BULLET)?;
    let head = rest.rsplit_once(REPLY_ARROW).map_or(rest, |(head, _hint)| head);

    let (name, tail) = match head.split_once(": ") {
        Some(parts) => parts,
        None => head.split_once(' ').unwrap_or((head, "")),
    };

    let (description, language) = split_language(tail);

    RecallScript::new(name, description, language)
}

/// `description (language)` as `(description, language)`; a text without a trailing `(…)` is all
/// description.
fn split_language(text: &str) -> (&str, &str) {
    let text = text.trim_end();

    let Some(without_paren) = text.strip_suffix(')') else {
        return (text, "");
    };

    match without_paren.rsplit_once('(') {
        Some((description, language)) => (description.trim_end(), language),
        None => (text, ""),
    }
}

/// `text` as one line: whitespace runs and control characters become a single space, then cut at a
/// character boundary to `max` bytes (an ellipsis marks the cut).
fn one_line(text: &str, max: usize) -> String {
    let joined = text
        .split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if joined.len() <= max {
        return joined;
    }

    let mut end = max.saturating_sub('…'.len_utf8());
    while !joined.is_char_boundary(end) {
        end -= 1;
    }

    format!("{}…", joined[..end].trim_end())
}
