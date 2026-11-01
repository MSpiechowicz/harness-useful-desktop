//! A minimal, line-based edit of OMP's `config.yml` that maps one Harness Skills role.
//!
//! The file is never re-serialized: lines are inserted into the existing `modelRoles` and
//! `task.agentModelOverrides` blocks (or the blocks are created at the end), so comments, order
//! and formatting stay byte-for-byte. The result must parse and must equal the old document plus
//! exactly the added keys, otherwise the edit is refused.

use serde_json::{Map, Value};

use crate::error::{BusinessError, Result};
use crate::line_edit::{Lines, content, is_skipped};
use crate::model::ConfigFormat;
use crate::pi_family::parse_text;

/// What to add.
pub struct Edit<'a> {
    /// The role to map (it must not be under `modelRoles` yet).
    pub role: &'a str,
    /// Its model selector.
    pub selector: &'a str,
    /// The agents that use the role (`@<role>` is added for those without an override).
    pub agents: Vec<&'a str>,
}

const MODEL_ROLES: &str = "modelRoles";
const OVERRIDES: [&str; 2] = ["task", "agentModelOverrides"];
const STEP: usize = 2;

/// The text of `config.yml` with the edit applied; the original is never modified.
pub fn apply_edit(text: &str, edit: &Edit<'_>) -> Result<String> {
    let old = parse(text)?;
    let missing_agents = missing_agents(&old, edit)?;

    let mut lines = Lines::new(text);
    lines.add_entries(&[MODEL_ROLES], &[(edit.role.to_owned(), edit.selector.to_owned())])?;

    if !missing_agents.is_empty() {
        let entries: Vec<(String, String)> = missing_agents
            .iter()
            .map(|agent| ((*agent).to_owned(), format!("\"@{}\"", edit.role)))
            .collect();

        lines.add_entries(&OVERRIDES, &entries)?;
    }

    let edited = lines.text();

    // Whatever the layout hid from the line search (a quoted key, say) shows up here.
    if parse(&edited).ok() != Some(expected(old, edit, &missing_agents)) {
        return Err(unsupported());
    }

    Ok(edited)
}

fn parse(text: &str) -> Result<Value> {
    let value = parse_text(text, ConfigFormat::Yaml)
        .map_err(|summary| BusinessError::InvalidInput(format!("the OMP settings are not valid YAML ({summary})")))?;

    if value.is_null() || value.is_object() {
        Ok(value)
    } else {
        Err(unsupported())
    }
}

fn unsupported() -> BusinessError {
    BusinessError::Unsupported("the OMP settings are laid out in a way that cannot be edited safely".to_owned())
}

/// The agents of the role that have no override yet; refuses a role that is already mapped.
fn missing_agents<'a>(old: &Value, edit: &Edit<'a>) -> Result<Vec<&'a str>> {
    let mapped = old.get(MODEL_ROLES);

    if mapped.is_some_and(|roles| !roles.is_null() && !roles.is_object()) {
        return Err(unsupported());
    }

    if mapped.and_then(|roles| roles.get(edit.role)).is_some() {
        return Err(BusinessError::Conflict(format!("{} is already mapped", edit.role)));
    }

    let overrides = old.get(OVERRIDES[0]).and_then(|task| task.get(OVERRIDES[1]));

    if old
        .get(OVERRIDES[0])
        .is_some_and(|task| !task.is_null() && !task.is_object())
        || overrides.is_some_and(|set| !set.is_null() && !set.is_object())
    {
        return Err(unsupported());
    }

    Ok(edit
        .agents
        .iter()
        .copied()
        .filter(|agent| overrides.and_then(|set| set.get(*agent)).is_none())
        .collect())
}

/// The old document with only the added keys put in.
fn expected(mut document: Value, edit: &Edit<'_>, agents: &[&str]) -> Value {
    object_at(&mut document, &[MODEL_ROLES]).insert(edit.role.to_owned(), Value::String(edit.selector.to_owned()));

    for agent in agents {
        object_at(&mut document, &OVERRIDES).insert((*agent).to_owned(), Value::String(format!("@{}", edit.role)));
    }

    document
}

/// The object at `path`, created (over a null) when missing.
fn object_at<'a>(document: &'a mut Value, path: &[&str]) -> &'a mut Map<String, Value> {
    let mut node = document;

    for key in path {
        if !node.is_object() {
            *node = Value::Object(Map::new());
        }

        let map = node.as_object_mut().expect("made an object above");
        node = map.entry((*key).to_owned()).or_insert(Value::Null);
    }

    if !node.is_object() {
        *node = Value::Object(Map::new());
    }

    node.as_object_mut().expect("made an object above")
}

impl Lines {
    /// Adds `entries` (`key`, rendered value) to the mapping at `path`, creating the part of the
    /// path that is missing at the end of its parent.
    fn add_entries(&mut self, path: &[&str], entries: &[(String, String)]) -> Result<()> {
        let mut span = Span {
            from: 0,
            to: self.lines.len(),
            indent: 0,
            after: self.lines.len().checked_sub(1),
        };

        for (depth, key) in path.iter().enumerate() {
            let Some(line) = self.find_key(&span, key)? else {
                self.create(&span, &path[depth..], entries);

                return Ok(());
            };

            span = self.block_of(line);
        }

        let indent = span.indent;
        let new: Vec<String> = entries
            .iter()
            .map(|(key, value)| format!("{}{key}: {value}", " ".repeat(indent)))
            .collect();

        self.insert_after(span.after, &new);

        Ok(())
    }

    /// The line of `key` at the level of `span`, if there is one.
    fn find_key(&self, span: &Span, key: &str) -> Result<Option<usize>> {
        for index in span.from..span.to {
            let line = content(&self.lines[index]);

            if is_skipped(line) {
                continue;
            }

            if indent_of(line)? != span.indent {
                continue;
            }

            let Some(rest) = line.trim_start().strip_prefix(key) else {
                continue;
            };
            let Some(rest) = rest.trim_start().strip_prefix(':') else {
                continue;
            };

            // `key: value` or `key: {}` is written inline and has no block to add to.
            let rest = rest.trim();

            if !(rest.is_empty() || rest.starts_with('#')) {
                return Err(unsupported());
            }

            return Ok(Some(index));
        }

        Ok(None)
    }

    /// The block that belongs to the key on `line`: its lines, the indent of its entries and the
    /// last line to insert after.
    fn block_of(&self, line: usize) -> Span {
        let key_indent = content(&self.lines[line]).len() - content(&self.lines[line]).trim_start().len();
        let mut last = line;
        let mut indent = None;
        let mut end = self.lines.len();

        for index in line + 1..self.lines.len() {
            let text = content(&self.lines[index]);

            if is_skipped(text) {
                continue;
            }

            let here = text.len() - text.trim_start().len();

            if here <= key_indent {
                end = index;
                break;
            }

            indent.get_or_insert(here);
            last = index;
        }

        Span {
            from: line + 1,
            to: end,
            indent: indent.unwrap_or(key_indent + STEP),
            after: Some(last),
        }
    }

    /// Appends the keys of `path` as nested blocks below `parent`, holding `entries`.
    fn create(&mut self, parent: &Span, path: &[&str], entries: &[(String, String)]) {
        let mut new = Vec::new();
        let mut indent = parent.indent;

        for key in path {
            new.push(format!("{}{key}:", " ".repeat(indent)));
            indent += STEP;
        }

        for (key, value) in entries {
            new.push(format!("{}{key}: {value}", " ".repeat(indent)));
        }

        self.insert_after(parent.after, &new);
    }

    /// Inserts `new` below line `after` (at the start when there is none), giving that line a
    /// terminator when it was the unterminated last line.
    fn insert_after(&mut self, after: Option<usize>, new: &[String]) {
        let at = after.map_or(0, |index| index + 1);
        let unterminated = at > 0 && !self.lines[at - 1].ends_with('\n');

        if unterminated {
            self.lines[at - 1].push_str(self.eol);
        }

        let eol = self.eol;
        let last = new.len() - 1;
        let added: Vec<String> = new
            .iter()
            .enumerate()
            .map(|(index, line)| {
                if unterminated && index == last {
                    line.clone()
                } else {
                    format!("{line}{eol}")
                }
            })
            .collect();

        self.lines.splice(at..at, added);
    }
}

/// Where a mapping lives in the lines.
struct Span {
    /// First line to look at.
    from: usize,
    /// One past the last.
    to: usize,
    /// Indent of the keys at this level.
    indent: usize,
    /// The line to insert after: the last line of the mapping, or of its parent key.
    after: Option<usize>,
}

fn indent_of(line: &str) -> Result<usize> {
    let indent = line.len() - line.trim_start_matches(' ').len();

    if line[indent..].starts_with('\t') {
        return Err(unsupported());
    }

    Ok(indent)
}
