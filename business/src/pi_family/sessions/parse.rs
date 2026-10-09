//! Streaming access to a session file: bounded JSONL lines and the logical entries.
//!
//! Lines are split on `\n` only, so U+2028/U+2029 stay inside JSON strings, and one trailing
//! `\r` is dropped (CRLF files). Lines that are too long, not UTF-8 or not JSON are skipped and
//! counted. The OMP title slot and the session header are not entries.
//!
//! Some entry types never reach the webview: `credential_pin` (OMP account pinning) and the
//! usage records (`usage`, `model_usage`) are spliced out of the tree. Their children are
//! re-parented to the nearest visible ancestor, so the branch structure survives.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Take};
use std::path::Path;

use serde_json::Value;

use crate::error::Result;
use crate::security::SafeReader;

/// Longest session line that is parsed; longer lines are skipped.
pub const MAX_LINE_BYTES: u64 = 32 * 1024 * 1024;

/// Entry types that are never surfaced (see the module docs).
const HIDDEN_TYPES: [&str; 3] = ["credential_pin", "usage", "model_usage"];

/// Entry types that are file metadata rather than entries.
const META_TYPES: [&str; 2] = ["title", "session"];

/// Lines of a session file, at most `budget` bytes in total.
pub struct Lines {
    reader: BufReader<Take<File>>,
    buffer: Vec<u8>,
    /// Lines that were skipped (too long or not UTF-8).
    pub skipped: usize,
}

impl Lines {
    /// Opens `path` through [`SafeReader`] and reads at most `budget` bytes.
    pub fn open(path: &Path, budget: u64) -> Result<Self> {
        let file = SafeReader::open(path)?;

        Ok(Self {
            reader: BufReader::new(file.take(budget)),
            buffer: Vec::new(),
            skipped: 0,
        })
    }

    /// Reads the next line into the buffer; `false` at the end (or on a read error).
    fn fill(&mut self) -> bool {
        self.buffer.clear();

        let limit = MAX_LINE_BYTES + 2;
        let Ok(read) = (&mut self.reader).take(limit).read_until(b'\n', &mut self.buffer) else {
            return false;
        };

        if read == 0 {
            return false;
        }

        let complete = self.buffer.last() == Some(&b'\n');
        if !complete && read as u64 >= limit {
            // Oversized: drop the rest of the line.
            self.reader.skip_until(b'\n').ok();
            self.buffer.clear();
            self.skipped += 1;
        }

        true
    }
}

impl Iterator for Lines {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        while self.fill() {
            if self.buffer.last() == Some(&b'\n') {
                self.buffer.pop();
            }

            if self.buffer.last() == Some(&b'\r') {
                self.buffer.pop();
            }

            if self.buffer.is_empty() {
                continue;
            }

            match String::from_utf8(std::mem::take(&mut self.buffer)) {
                Ok(line) => return Some(line),
                Err(_) => self.skipped += 1,
            }
        }

        None
    }
}

/// One logical session entry with its parent resolved past hidden entries.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Entry id.
    pub id: String,
    /// Visible parent id.
    pub parent_id: Option<String>,
    /// The parsed line.
    pub value: Value,
}

impl Entry {
    /// The entry type (`message`, `compaction`, ...).
    pub fn kind(&self) -> &str {
        self.value.get("type").and_then(Value::as_str).unwrap_or_default()
    }

    /// The nested `AgentMessage` of a `message` entry.
    pub fn message(&self) -> Option<&Value> {
        (self.kind() == "message").then(|| self.value.get("message")).flatten()
    }

    /// The `role` of a `message` entry.
    pub fn role(&self) -> Option<&str> {
        self.message()?.get("role").and_then(Value::as_str)
    }

    /// A string field of the entry.
    pub fn text(&self, key: &str) -> Option<&str> {
        self.value.get(key).and_then(Value::as_str)
    }
}

/// The visible entries of a session file in file order.
pub struct Entries {
    lines: Lines,
    hidden: HashMap<String, Option<String>>,
    seen: HashSet<String>,
    /// Lines that were not valid entries (malformed JSON, missing id, duplicates).
    pub malformed: usize,
}

impl Entries {
    /// Streams the entries of `path` (no byte budget beyond the per-line cap).
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            lines: Lines::open(path, u64::MAX)?,
            hidden: HashMap::new(),
            seen: HashSet::new(),
            malformed: 0,
        })
    }

    /// Skipped and malformed lines so far.
    pub fn bad_lines(&self) -> usize {
        self.malformed + self.lines.skipped
    }
}

/// `parent` resolved past hidden entries.
fn visible_parent(hidden: &HashMap<String, Option<String>>, parent: Option<String>) -> Option<String> {
    let mut current = parent;

    // Hidden entries always point at earlier entries, so the walk ends; the bound guards against
    // corrupt files with cycles.
    for _ in 0..=hidden.len() {
        match current {
            Some(ref id) if hidden.contains_key(id) => current = hidden[id].clone(),
            _ => return current,
        }
    }

    None
}

impl Iterator for Entries {
    type Item = Entry;

    fn next(&mut self) -> Option<Entry> {
        for line in self.lines.by_ref() {
            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                self.malformed += 1;
                continue;
            };

            let kind = value.get("type").and_then(Value::as_str).unwrap_or_default();
            if META_TYPES.contains(&kind) {
                continue;
            }

            let Some(id) = value.get("id").and_then(Value::as_str).map(str::to_owned) else {
                self.malformed += 1;
                continue;
            };

            if !self.seen.insert(id.clone()) {
                self.malformed += 1;
                continue;
            }

            let parent = value.get("parentId").and_then(Value::as_str).map(str::to_owned);
            let parent_id = visible_parent(&self.hidden, parent);

            if HIDDEN_TYPES.contains(&kind) {
                self.hidden.insert(id, parent_id);
                continue;
            }

            return Some(Entry { id, parent_id, value });
        }

        None
    }
}

/// The text of message content (a string or text blocks), whitespace collapsed, cut to
/// `max_chars` characters (an ellipsis marks the cut); `None` when empty.
pub fn preview_text(content: Option<&Value>, max_chars: usize) -> Option<String> {
    let pieces: Vec<&str> = match content? {
        Value::String(raw) => vec![raw.as_str()],
        Value::Array(blocks) => blocks
            .iter()
            .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect(),
        _ => Vec::new(),
    };

    // Words beyond the limit are not needed; the bound keeps huge prompts cheap.
    let words: Vec<&str> = pieces
        .iter()
        .flat_map(|piece| piece.split_whitespace())
        .take(max_chars + 1)
        .collect();
    let text = words.join(" ");

    if text.is_empty() {
        return None;
    }

    Some(cut(&text, max_chars))
}

/// `text` cut to at most `max_chars` characters, the last one an ellipsis when cut.
pub fn cut(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }

    let mut kept: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    kept.truncate(kept.trim_end().len());
    kept.push('…');
    kept
}

/// Milliseconds since the Unix epoch of an RFC 3339 timestamp
/// (`YYYY-MM-DDTHH:MM:SS[.fff](Z|±HH:MM)`).
pub fn epoch_millis(timestamp: &str) -> Option<i64> {
    let bytes = timestamp.as_bytes();
    let number = |range: std::ops::Range<usize>| -> Option<i64> { timestamp.get(range)?.parse().ok() };

    if bytes.len() < 20 || bytes[4] != b'-' || bytes[7] != b'-' || !matches!(bytes[10], b'T' | b't' | b' ') {
        return None;
    }

    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);

    let mut rest = &timestamp[19..];
    let mut millis = 0;

    if let Some(fraction) = rest.strip_prefix('.') {
        let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
        let padded = format!("{:0<3}", &fraction[..digits.min(3)]);
        millis = padded.parse::<i64>().ok()?;
        rest = &fraction[digits..];
    }

    let offset_minutes = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let hours: i64 = rest.get(1..3)?.parse().ok()?;
            let minutes: i64 = rest.get(4..6)?.parse().ok()?;
            sign * (hours * 60 + minutes)
        }
    };

    let days = days_from_civil(year, month, day);
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second - offset_minutes * 60;

    Some(seconds * 1000 + millis)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    era * 146_097 + day_of_era - 719_468
}
