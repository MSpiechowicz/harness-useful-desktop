//! The user-only save commands of a server with `save-guards` (`notes save|update`, `scripts
//! save`, `held list|save|skip`), answered from what they read on stdin and fixed fixtures; nothing
//! is kept between runs.
//!
//! - `notes save --json [--client NAME] [--project P]` prints `{"id": 77, "title", "scope"}`; a draft titled
//!   [`TAKEN_TITLE`] is refused `refused` with `note 3 already has this title …` (exit 1).
//! - `notes update <id> --json [--client NAME] [--project P]` prints `{"id", "title"}`; id [`MISSING_ID`] is
//!   refused `notFound`.
//! - `scripts save --json [--client NAME] [--project P]` prints `{"name", "scope", "id": 12, "created": false,
//!   "approval": "local"}`.
//! - `held list --json --chat KEY` prints `{"held": [...]}`: for chat [`CHAT`] [`HELD_NOTE`], a
//!   held `note_save` at revision [`REVISION`], and an entry of another version that the app
//!   skips; for chat [`BIG_CHAT`] one held note of [`BIG_BODY_BYTES`]; nothing for another chat.
//! - `held save <id> --json [--expect REVISION]` prints `{"saved": {"kind": "note", "id": 12}}`
//!   for [`HELD_NOTE`] (refused `changed` when the revision expected is not [`REVISION`]) and is
//!   refused `gone` for [`HELD_GONE`]; `held skip <id> --json [--expect REVISION]` prints
//!   `{"skipped": id}` (refused `changed` as save); any other id is refused `notFound`.
//!
//! Any other shape is a usage error (exit 2).

use std::io::Read;

use serde_json::{Value, json};

/// The title another note has.
const TAKEN_TITLE: &str = "Taken title";

/// The note `notes update` does not find.
const MISSING_ID: &str = "404";

/// The chat of the held saves.
const CHAT: &str = "3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b";

/// The one held save.
const HELD_NOTE: &str = "6c1f3d2a-9b8e-4f7a-8c6d-5e4f3a2b1c0d";

/// A held note change whose note was deleted.
const HELD_GONE: &str = "8e3f5a4c-1d0a-4b9c-8e7f-7a6b5c4d3e2f";

/// The revision of [`HELD_NOTE`].
const REVISION: &str = "5f1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f9";

/// The chat whose one held note is larger than the output most commands keep.
const BIG_CHAT: &str = "9a8b7c6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d";

/// The body of the held note of [`BIG_CHAT`] (3 MiB).
const BIG_BODY_BYTES: usize = 3 * 1024 * 1024;

/// `harness-memory notes save|update ...`, when it is one.
pub fn notes(args: &[String]) {
    match strs(args).as_slice() {
        ["save", "--json", rest @ ..] if save_options(rest) => {
            let draft = stdin_json();

            if draft["title"] == TAKEN_TITLE {
                refuse(
                    "refused",
                    "note 3 already has this title and was written from another checkout",
                );
            }

            answer(json!({ "id": 77, "title": draft["title"], "scope": draft["scope"] }));
        }
        ["update", id, "--json", rest @ ..] if save_options(rest) => {
            let draft = stdin_json();

            if *id == MISSING_ID {
                refuse("notFound", &format!("note {id} not found"));
            }

            answer(json!({ "id": id.parse::<i64>().unwrap_or_default(), "title": draft["title"] }));
        }
        _ => {}
    }
}

/// `harness-memory scripts save ...`, when it is one.
pub fn scripts(args: &[String]) {
    if let ["save", "--json", rest @ ..] = strs(args).as_slice()
        && save_options(rest)
    {
        let draft = stdin_json();
        answer(json!({
            "name": draft["name"], "scope": draft["scope"], "id": 12, "created": false, "approval": "local",
        }));
    }
}

/// `harness-memory held <args>`.
pub fn held(args: &[String]) -> ! {
    match strs(args).as_slice() {
        ["list", "--json", "--chat", CHAT] => answer(json!({ "held": [held_note(), newer()] })),
        ["list", "--json", "--chat", BIG_CHAT] => answer(json!({ "held": [big_note()] })),
        ["list", "--json", "--chat", _] => answer(json!({ "held": [] })),
        ["save", HELD_NOTE, "--json"] | ["save", HELD_NOTE, "--json", "--expect", REVISION] => {
            answer(json!({ "saved": { "kind": "note", "id": 12 } }))
        }
        ["save", HELD_NOTE, "--json", "--expect", _] => refuse("changed", "the held save changed"),
        ["save", HELD_GONE, "--json"] => refuse("gone", "note 4 was deleted"),
        ["skip", HELD_NOTE, "--json"] | ["skip", HELD_NOTE, "--json", "--expect", REVISION] => {
            answer(json!({ "skipped": HELD_NOTE }))
        }
        ["skip", HELD_NOTE, "--json", "--expect", _] => refuse("changed", "the held save changed"),
        ["save" | "skip", id, "--json"] => refuse("notFound", &format!("no held save {id}")),
        _ => {
            eprintln!("usage: harness-memory held list|save|skip");
            std::process::exit(2);
        }
    }
}

fn held_note() -> Value {
    json!({
        "version": 1, "heldId": HELD_NOTE, "chatKey": CHAT, "createdAt": "2026-10-21T10:00:00Z",
        "client": "omp", "kind": "note", "tool": "note_save", "projectPath": "/work/repo",
        "repoRoot": "/work/repo", "revision": REVISION,
        "arguments": { "title": "Retry the flaky build", "body": "Twice.", "kind": "fix", "tags": [] },
        "current": null,
    })
}

fn big_note() -> Value {
    let mut note = held_note();
    note["chatKey"] = json!(BIG_CHAT);
    note["arguments"]["body"] = json!("a".repeat(BIG_BODY_BYTES));

    note
}

fn newer() -> Value {
    json!({ "version": 2, "heldId": "7d2e4f3b-0c9f-4a8b-9d7e-6f5a4b3c2d1e" })
}

fn strs(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}

/// Whether `rest` is an optional `--client <name>` then an optional `--project <path>`.
fn save_options(rest: &[&str]) -> bool {
    matches!(
        rest,
        [] | ["--project", _] | ["--client", _] | ["--client", _, "--project", _]
    )
}

fn stdin_json() -> Value {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).ok();

    serde_json::from_str(&text).unwrap_or(Value::Null)
}

fn answer(value: Value) -> ! {
    println!("{value}");
    std::process::exit(0);
}

fn refuse(code: &str, message: &str) -> ! {
    println!("{}", json!({ "error": code, "message": message }));
    eprintln!("harness-memory: {message}");
    std::process::exit(1);
}
