//! YAML frontmatter of Markdown resources (`SKILL.md`, prompt templates).
//!
//! Only the block between a leading `---` line and the next `---` line is parsed; the body is
//! never returned.

use std::path::Path;

use serde_json::Value;

use crate::security::SafeReader;

/// Size cap for Markdown resources (skills and templates are small text files).
pub const MAX_MARKDOWN_BYTES: u64 = 256 * 1024;

/// Largest frontmatter block parsed; real ones hold a few fields, and YAML parsing is the slow
/// part of a scan.
pub const MAX_FRONTMATTER_BYTES: usize = 64 * 1024;

/// The frontmatter of the file at `path` as a JSON object; `None` when the file is unreadable, has
/// no frontmatter or the frontmatter is not a YAML mapping. Read fields with
/// [`super::text_field`].
pub fn read_frontmatter(path: &Path) -> Option<Value> {
    let text = SafeReader::read_to_string(path, MAX_MARKDOWN_BYTES).ok()?;
    parse_frontmatter(&text)
}

/// Parses the frontmatter block of `text` (see the module docs); `None` for a block larger than
/// [`MAX_FRONTMATTER_BYTES`].
pub fn parse_frontmatter(text: &str) -> Option<Value> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();

    if lines.next()?.trim_end() != "---" {
        return None;
    }

    let mut block = Vec::new();
    let mut block_bytes = 0;
    let mut closed = false;

    for line in lines {
        if line.trim_end() == "---" {
            closed = true;
            break;
        }

        block_bytes += line.len() + 1;
        if block_bytes > MAX_FRONTMATTER_BYTES {
            return None;
        }

        block.push(line);
    }

    if !closed {
        return None;
    }

    serde_norway::from_str::<Value>(&block.join("\n"))
        .ok()
        .filter(Value::is_object)
}
