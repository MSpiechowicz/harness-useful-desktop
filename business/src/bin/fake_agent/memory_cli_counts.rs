//! The counts a list of the `harness-memory notes list` and `scripts list` stand-ins
//! ([`crate::memory_cli`]) answers: the notes of each kind, the entries of each scope, the scripts
//! of each group and review state.

use serde_json::{Value, json};

/// Whether `entry` is of note kind or script group `filter`. Script groups are disjoint: `repo` and `global` hold only trusted scripts.
pub fn in_group(entry: &Value, filter: &str) -> bool {
    match filter {
        "needsReview" => entry["trusted"] == false,
        "repo" | "global" => entry["trusted"] != false && entry["scope"] == filter,
        kind => entry["kind"] == kind,
    }
}

/// `needed` for a script that needs approval, else `trusted`.
pub fn review_of(entry: &Value) -> &'static str {
    if entry["trusted"] == false { "needed" } else { "trusted" }
}

pub fn scope_counts(entries: &[&Value]) -> Value {
    let count = |scope: &str| entries.iter().filter(|entry| entry["scope"] == scope).count();

    json!({ "repo": count("repo"), "global": count("global") })
}

pub fn kind_counts(notes: &[Value]) -> Value {
    let mut kinds: Vec<Value> = Vec::new();

    for note in notes {
        match kinds.iter_mut().find(|entry| entry["kind"] == note["kind"]) {
            Some(entry) => entry["count"] = json!(entry["count"].as_u64().unwrap_or_default() + 1),
            None => kinds.push(json!({ "kind": note["kind"], "count": 1 })),
        }
    }

    json!(kinds)
}

pub fn group_counts(scripts: &[Value]) -> Value {
    let count = |group: &str| scripts.iter().filter(|script| in_group(script, group)).count();

    json!({ "needsReview": count("needsReview"), "repo": count("repo"), "global": count("global") })
}
