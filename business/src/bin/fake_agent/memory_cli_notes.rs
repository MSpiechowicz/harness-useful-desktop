//! The fixed notes of the `harness-memory notes` stand-in ([`crate::memory_cli`]): the notes and the
//! links between them, and how often they were shown to an agent and found useful.

use serde_json::{Value, json};

/// What `--project` adds: the repository the stand-in's repository notes belong to.
const REPO: &str = "github.com/o/r";

/// How often the notes were shown to an agent, and found useful (the last time, when): id, shown,
/// useful, last useful. A note not listed (a fixture added by `FAKE_MEMORY_NOTES`, a hidden one)
/// was never shown. Notes 3, 5 and 8 are shown often and never used, 6 not often enough yet.
const USEFULNESS: [(i64, u32, u32, Option<&str>); 8] = [
    (1, 12, 9, Some("2026-10-09T09:00:00Z")),
    (2, 8, 5, Some("2026-10-08T09:00:00Z")),
    (3, 6, 0, None),
    (4, 4, 1, Some("2026-10-02T09:00:00Z")),
    (5, 15, 0, None),
    (6, 2, 0, None),
    (7, 9, 7, Some("2026-10-03T09:00:00Z")),
    (8, 3, 0, None),
];

/// `note` with the `shown`, `useful` and `lastUsefulAt` of [`USEFULNESS`].
pub fn with_usefulness(mut note: Value) -> Value {
    let id = note["id"].as_i64().unwrap_or_default();
    let (shown, useful, last) = USEFULNESS
        .iter()
        .find(|row| row.0 == id)
        .map_or((0, 0, None), |row| (row.1, row.2, row.3));

    note["shown"] = json!(shown);
    note["useful"] = json!(useful);
    note["lastUsefulAt"] = last.map_or(Value::Null, |at| json!(at));

    note
}

/// The links between fixtures: from, to, relation.
pub const LINKS: [(i64, i64, &str); 3] = [(2, 3, "fixes"), (5, 2, "follows"), (6, 1, "explains")];

pub fn link(notes: &[Value], other: i64, relation: &str, direction: &str) -> Option<Value> {
    let note = notes.iter().find(|note| note["id"] == other)?;

    Some(json!({
        "id": other,
        "title": note["title"],
        "kind": note["kind"],
        "relation": relation,
        "direction": direction,
    }))
}

/// The stand-in's notes: id, kind, title, scope, place and update time.
const NOTES: [&str; 8] = [
    "1|decision|Use rustls everywhere|global|team|2026-10-09T10:00:00Z",
    "2|fix|Retry the watcher on rename|repo|team|2026-10-08T10:00:00Z",
    "3|problem|Watcher misses renames|repo|local|2026-10-07T10:00:00Z",
    "4|outcome|Release 0.19 shipped|global|team|2026-10-06T10:00:00Z",
    "5|optimisation|Cache the tools probe|repo|local|2026-10-05T10:00:00Z",
    "6|location|Memory config lives in the config dir|global|local|2026-10-04T10:00:00Z",
    "7|note|Prefer bun over npm|global|local|2026-10-03T10:00:00Z",
    "8|idea|A kind from a newer server|global|local|2026-10-02T10:00:00Z",
];

/// The stand-in's notes, with their bodies.
pub fn note_fixtures() -> Vec<Value> {
    let note = |id: i64, kind: &str, title: &str, scope: &str, place: &str, updated: &str| {
        json!({
            "id": id,
            "kind": kind,
            "title": title,
            "scope": scope,
            "repo": if scope == "repo" { json!(REPO) } else { Value::Null },
            "place": place,
            "tags": [kind, "fixture"],
            "entities": [format!("src/{kind}.rs")],
            "author": { "source": "claude-code", "root": null },
            "createdAt": "2026-10-01T08:00:00Z",
            "updatedAt": updated,
            "revision": id % 3 + 1,
            "hits": id * 2,
            "body": format!("{title}.\nWritten by an agent.\n"),
        })
    };

    let mut notes: Vec<Value> = NOTES
        .iter()
        .map(|row| {
            let fields: Vec<&str> = row.split('|').collect();
            let id = fields[0].parse().unwrap_or_default();
            note(id, fields[1], fields[2], fields[3], fields[4], fields[5])
        })
        .collect();

    let added = std::env::var("FAKE_MEMORY_NOTES").unwrap_or_default();

    for (id, title) in (100..).zip(added.lines()) {
        notes.push(note(id, "note", title, "global", "local", "2026-09-01T10:00:00Z"));
    }

    notes
}
