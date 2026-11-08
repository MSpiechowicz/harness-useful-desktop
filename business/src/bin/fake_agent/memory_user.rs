//! The user-only save commands of a server with `save-guards` (`notes save|update`, `scripts
//! save`, `held list|save|skip`), answered from what they read on stdin and fixed fixtures; nothing
//! is kept between runs.
//!
//! - `notes save --json [--client NAME] [--project P] [--place remote|local]` prints `{"id": 77, "title", "scope"}`,
//!   with `"place"` when one was given; `--place` is a usage error unless the stand-in lists `save-place`. A draft
//!   titled [`TAKEN_TITLE`] is refused `refused` with `note 3 already has this title …` (exit 1). With
//!   `FAKE_MEMORY_REMOTE_DOWN` set (any value) the team database cannot be reached: a save to `--place remote` is
//!   refused `unreachable` ([`REMOTE_DOWN_MESSAGE`], exit 1) and saves nothing.
//! - `notes update <id> --json [--client NAME] [--project P] [--expect-revision N]` prints `{"id", "title"}`;
//!   id [`MISSING_ID`] is refused `notFound`, id [`MOVED_ID`] `changed` with `"reason": "project"`. With `--expect-revision` (a stand-in that lists
//!   `note-expect-revision`, else a usage error) a revision other than [`NOTE_REVISION`] is refused `changed`.
//!   With `FAKE_MEMORY_QUEUED` set (any value) the team database is unreachable: the change is kept on this
//!   computer and answered `{"id", "title", "queued": true, "message"}`, `title` null when the draft gives none.
//! - `scripts save --json [--client NAME] [--project P] [--place remote|local]` prints `{"name", "scope", "id": 12,
//!   "created": false, "approval": "local"}`, with `"place"` as `notes save`, and is refused the same way.
//! - `held list --json --chat KEY` prints `{"held": [...]}`: for chat [`CHAT`] [`HELD_NOTE`], a
//!   held `note_save` at revision [`REVISION`], and an entry of another version that the app
//!   skips; for chat [`BIG_CHAT`] one held note of [`BIG_BODY_BYTES`]; nothing for another chat.
//! - `held save <id> --json [--expect REVISION]` prints `{"saved": {"kind": "note", "id": 12}}`
//!   for [`HELD_NOTE`] (refused `changed` when the revision expected is not [`REVISION`]) and is
//!   refused `gone` for [`HELD_GONE`], and prints `{"saved": {"kind": "note", "id": 4}}` for [`HELD_UPDATE`], a held `note_update`.
//!   `--place remote|local` after them is a usage error unless the stand-in lists `held-save-place`, and a held
//!   `note_update` refuses it (`invalid`); `held skip <id> --json [--expect REVISION]` prints
//!   `{"skipped": id}` (refused `changed` as save); any other id is refused `notFound`.
//!
//! Any other shape is a usage error (exit 2).

use std::io::Read;

use serde_json::{Value, json};

use crate::memory_push::lists;

/// The title another note has.
const TAKEN_TITLE: &str = "Taken title";

/// The note `notes update` does not find.
const MISSING_ID: &str = "404";

/// The note `notes update` refuses `changed` for its project, not for its revision.
const MOVED_ID: &str = "405";

/// What a change kept while the team database is unreachable is told to say.
const QUEUED_MESSAGE: &str = "saved on this computer, applied when the remote database is back";

/// Whether the stand-in plays a server whose team database cannot be reached (`FAKE_MEMORY_QUEUED`).
fn queued() -> bool {
    std::env::var_os("FAKE_MEMORY_QUEUED").is_some()
}

/// What the team database being unreachable is told to say (it carries a password, which the app
/// redacts).
pub const REMOTE_DOWN_MESSAGE: &str =
    "the team database cannot be reached: postgres://user:hunter2@db.example.com/memory refused the connection";

/// Whether the stand-in plays a server whose team database cannot be reached (`FAKE_MEMORY_REMOTE_DOWN`).
fn remote_down() -> bool {
    std::env::var_os("FAKE_MEMORY_REMOTE_DOWN").is_some()
}

/// The revision every note `notes update` finds is at.
const NOTE_REVISION: &str = "4";

/// The chat of the held saves.
const CHAT: &str = "3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b";

/// The one held save.
const HELD_NOTE: &str = "6c1f3d2a-9b8e-4f7a-8c6d-5e4f3a2b1c0d";

/// A held `note_update` (not listed, only saved).
const HELD_UPDATE: &str = "9d4e6f5b-2e1b-4c0d-9f8a-8b7c6d5e4f30";

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
            let place = place_of(rest);
            let draft = stdin_json();

            refuse_unreachable(place);

            if draft["title"] == TAKEN_TITLE {
                refuse(
                    "refused",
                    "note 3 already has this title and was written from another checkout",
                );
            }

            answer(with_place(
                json!({ "id": 77, "title": draft["title"], "scope": draft["scope"] }),
                place,
            ));
        }
        ["update", id, "--json", rest @ ..] => {
            let Some(expected) = update_options(rest) else {
                return;
            };

            let draft = stdin_json();

            if *id == MISSING_ID {
                refuse("notFound", &format!("note {id} not found"));
            }

            if *id == MOVED_ID {
                refuse_with(json!({
                    "error": "changed",
                    "reason": "project",
                    "message": "the folder this note is saved for is gone, so nothing was saved",
                }));
            }

            if let Some(revision) = expected {
                if !lists("note-expect-revision") {
                    usage();
                }

                if revision != NOTE_REVISION {
                    refuse("changed", "the note changed since the revision expected");
                }
            }

            let id = id.parse::<i64>().unwrap_or_default();

            if queued() {
                answer(json!({ "id": id, "title": draft["title"], "queued": true, "message": QUEUED_MESSAGE }));
            }

            answer(json!({ "id": id, "title": draft["title"] }));
        }
        _ => {}
    }
}

/// `harness-memory scripts save ...`, when it is one.
pub fn scripts(args: &[String]) {
    if let ["save", "--json", rest @ ..] = strs(args).as_slice()
        && save_options(rest)
    {
        let place = place_of(rest);
        let draft = stdin_json();

        refuse_unreachable(place);

        answer(with_place(
            json!({
                "name": draft["name"], "scope": draft["scope"], "id": 12, "created": false, "approval": "local",
            }),
            place,
        ));
    }
}

/// `harness-memory held <args>`.
pub fn held(args: &[String]) -> ! {
    let args = strs(args);

    // `--place remote|local` closes a `held save`; it is the same save without it, refused for a held `note_update`.
    let (args, place) = match args.as_slice() {
        [options @ .., "--place", place @ ("remote" | "local")] if options.first() == Some(&"save") => {
            if !lists("held-save-place") {
                usage();
            }

            (options.to_vec(), Some(*place))
        }
        options => (options.to_vec(), None),
    };

    if place.is_some() && matches!(args.as_slice(), ["save", HELD_UPDATE, ..]) {
        refuse(
            "invalid",
            "a held note_update keeps its note where it is, so it takes no place",
        );
    }

    match args.as_slice() {
        ["save", HELD_UPDATE, "--json"] => answer(json!({ "saved": { "kind": "note", "id": 4 } })),
        ["list", "--json", "--chat", CHAT] => answer(json!({ "held": [held_note(), newer()] })),
        ["list", "--json", "--chat", BIG_CHAT] => answer(json!({ "held": [big_note()] })),
        ["list", "--json", "--chat", _] => answer(json!({ "held": [] })),
        ["save", HELD_NOTE, "--json"] | ["save", HELD_NOTE, "--json", "--expect", REVISION] if queued() => {
            answer(json!({ "saved": { "kind": "note", "id": 12, "queued": true, "message": QUEUED_MESSAGE } }))
        }
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

fn usage() -> ! {
    eprintln!("usage: harness-memory notes update <id> --json");
    std::process::exit(2);
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

/// Whether `rest` is an optional `--client <name>`, then an optional `--project <path>`, then an
/// optional `--place remote|local` (see [`without_place`]).
fn save_options(rest: &[&str]) -> bool {
    update_base(without_place(rest))
}

/// Whether `rest` is an optional `--client <name>` then an optional `--project <path>`.
fn update_base(rest: &[&str]) -> bool {
    matches!(
        rest,
        [] | ["--project", _] | ["--client", _] | ["--client", _, "--project", _]
    )
}

/// `rest` without a trailing `--place remote|local`.
fn without_place<'a, 'b>(rest: &'b [&'a str]) -> &'b [&'a str] {
    match rest {
        [options @ .., "--place", "remote" | "local"] => options,
        options => options,
    }
}

/// The place `rest` names; a usage error unless the stand-in lists `save-place`.
fn place_of<'a>(rest: &[&'a str]) -> Option<&'a str> {
    let [.., "--place", place] = rest else {
        return None;
    };

    if !lists("save-place") {
        usage();
    }

    Some(place)
}

/// `data` with the `place` the save went to, when one was named.
fn with_place(mut data: Value, place: Option<&str>) -> Value {
    if let Some(place) = place {
        data["place"] = json!(place);
    }

    data
}

/// Refuses a save to the team database while it cannot be reached.
fn refuse_unreachable(place: Option<&str>) {
    if place == Some("remote") && remote_down() {
        refuse("unreachable", REMOTE_DOWN_MESSAGE);
    }
}

/// The revision expected (`None` when none is given) when `rest` is the options of `notes update`:
/// those of a save without a place, then an optional `--expect-revision N`; `None` for any other shape.
fn update_options<'a>(rest: &[&'a str]) -> Option<Option<&'a str>> {
    match rest {
        [options @ .., "--expect-revision", revision] if update_base(options) => Some(Some(*revision)),
        options if update_base(options) => Some(None),
        _ => None,
    }
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
    refuse_with(json!({ "error": code, "message": message }))
}

/// Refuses with `answer` (its `message` is also the line on stderr).
fn refuse_with(answer: Value) -> ! {
    println!("{answer}");
    eprintln!("harness-memory: {}", answer["message"].as_str().unwrap_or_default());
    std::process::exit(1);
}
