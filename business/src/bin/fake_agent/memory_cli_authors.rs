//! The author filter of the `harness-memory notes list` and `scripts list` stand-ins
//! ([`crate::memory_cli`]): `--author NAME`, `--author-unknown`, `--sort addedBy` and the `authors`
//! facet of the answer, all usage errors unless the stand-in lists `notes-author-filter`.

use serde_json::{Value, json};

/// The most characters of an author's name, as the server takes.
const NAME_MAX: usize = 80;

/// Whether this stand-in lists `notes-author-filter`.
pub fn listed() -> bool {
    crate::memory_push::lists("notes-author-filter")
}

/// `raw` as the server takes it for `--author`: not blank, trimmed, at most [`NAME_MAX`]
/// characters, no control character. Anything else is a usage error (exit 2), as the real server
/// answers it.
pub fn checked_name(raw: String) -> String {
    let fit =
        !raw.is_empty() && raw.trim() == raw && raw.chars().count() <= NAME_MAX && !raw.chars().any(char::is_control);

    if !fit {
        crate::memory_cli::usage();
    }

    raw
}

/// The name `entry` was first added by, when one is known.
fn added_by(entry: &Value) -> Option<&str> {
    entry["addedBy"]["name"].as_str()
}

/// Whether `entry` is kept by `--author NAME` (`author`) or `--author-unknown` (`unknown`).
pub fn keeps(entry: &Value, author: Option<&str>, unknown: bool) -> bool {
    match (author, unknown) {
        (Some(name), _) => added_by(entry) == Some(name),
        (None, true) => added_by(entry).is_none(),
        (None, false) => true,
    }
}

/// What `--sort addedBy` orders `entry` by.
pub fn sort_value(entry: &Value) -> &Value {
    &entry["addedBy"]["name"]
}

/// The `authors` facet of `entries`, the ones under every filter but the author's: people with
/// the most entries first, then by name, and how many have no name.
pub fn facet(entries: &[&Value]) -> Value {
    let mut people: Vec<(&str, usize)> = Vec::new();

    for name in entries.iter().filter_map(|entry| added_by(entry)) {
        match people.iter_mut().find(|(found, _)| *found == name) {
            Some((_, count)) => *count += 1,
            None => people.push((name, 1)),
        }
    }

    people.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

    let unknown = entries.iter().filter(|entry| added_by(entry).is_none()).count();
    let people: Vec<Value> = people
        .into_iter()
        .map(|(name, count)| json!({ "name": name, "count": count }))
        .collect();

    json!({ "people": people, "unknown": unknown })
}
