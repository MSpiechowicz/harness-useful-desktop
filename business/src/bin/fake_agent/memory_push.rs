//! `harness-memory features` (the module `listed`), `harness-memory push --json`, `harness-memory
//! pull --json` (the module `pulled`), `harness-memory notes hide|unhide|discard-local` stand-ins and
//! the team-and-local fields of `notes list|get`, answered from fixed fixtures (nothing is kept
//! between runs).
//!
//! - Each note of `notes list|get` gets `readOnly` (a team note of a scope routed `remote+local` by
//!   `HARNESS_MEMORY_REPOSITORY_NOTES` or `HARNESS_MEMORY_GLOBAL_NOTES`, as the real server reads
//!   them) and `hidden`; a list adds `hiddenCount` (1) and `localCounts` ([`LOCAL`] by scope).
//!   `--hidden` lists, or gets, the one hidden team note ([`HIDDEN_ID`]) instead.
//! - `notes hide|unhide <id> --json [--project P]` answers `{"hidden"|"unhidden": <id>}` for a team
//!   note it can see, refuses a local one and an unknown one (exit 1, the real server's lines).
//! - `notes discard-local (--all | - | IDS) [--scope S] --yes --json` answers `{"deleted": N}`:
//!   every local fixture of the scope with `--all`, else the ids given (`-`: on stdin). Without
//!   `--yes` it is a usage error.
//! - `push --json [--dry-run] [--overwrite none|all|-|IDS | --selection -] [--delete-copied]
//!   [--scope S] [--remote-local]` answers the [`LOCAL`] notes of the scope as the real server does: without
//!   `--scope`, those of every scope not routed `local` or `remote+local` (by the variables above),
//!   and with `--remote-local` (a usage error unless the stand-in lists `push-remote-local`) those of
//!   the scopes routed `remote+local` and no other; conflicts are overwritten
//!   when `--overwrite` names them (`-`: ids on stdin), `--selection -` (a usage error unless the
//!   stand-in lists `push-selection`, and with `--overwrite`) reads `{"only": [{"id", "name",
//!   "updatedAt"}, ...], "overwrite": [ids]}` from stdin and takes only the entries of `only` whose
//!   title or name and update time still are those of the entry (the update time of entry `id`
//!   is `2026-10-10T10:00:<id>.250000Z`), overwriting those of `overwrite`; an entry that differs is
//!   not copied or deleted but marked `"failed": true` with an entry of `failed`, and an `only` of
//!   bare ids is a usage error. A real run fails [`NOTES`]'s failing
//!   note (marked `"failed": true` in `changes`, exit 1, a URL with a password in its message).
//!   `scripts push` takes the same options and answers the [`LOCAL_SCRIPTS`] the same way (a `name`
//!   for a `title`, a `scripts` counts object for `notes`), routed by `HARNESS_MEMORY_SCRIPTS`.
//!   For both, `FAKE_MEMORY_PUSH=nothing` has nothing to push, `FAKE_MEMORY_PUSH=usage` predates
//!   `push --json` (exit 2), `FAKE_MEMORY_PUSH=unflagged` predates the `failed` field of
//!   `changes`, and `FAKE_MEMORY_PUSH=relation` also fails a relation on a real run (a failure
//!   with a null `localId`) and so deletes nothing, and `FAKE_MEMORY_PUSH=undeleted` copies the
//!   first entry on a real run with `--delete-copied` but fails to delete its local copy (an entry
//!   of `failed` with its `localId`, its change neither `deleted` nor `failed`).
//!   `FAKE_MEMORY_PUSH=hidden` matches the fourth entry (a conflict) with a team entry hidden on
//!   this computer: a dry run and a real run alike mark it `"failed": true` with an entry of
//!   `failed` (exit 1, a URL with a password in its message). `FAKE_MEMORY_PUSH=changed` copies the
//!   second entry on a real run with `--delete-copied` but keeps its local copy, changed during the
//!   push (`"changedSince": true`, not `deleted`, no entry of `failed`). `FAKE_MEMORY_PUSH=late`
//!   leaves the sixth entry out of a dry run, as an entry saved after the review: a real run takes
//!   it unless `--selection` leaves it out of `only`. `FAKE_MEMORY_PUSH=stale` changes the update time
//!   of the first entry and `FAKE_MEMORY_PUSH=renamed` its title or name on a real run, as an entry
//!   saved again after the review, and `FAKE_MEMORY_PUSH=undated` prints no `localUpdatedAt` for the
//!   first entry.
//! - An agent's new note or script of a scope routed `remote+local` says where it went (see
//!   [`agent_note_place`]).

use std::io::Read;

use serde_json::{Value, json};

#[path = "memory_features.rs"]
mod listed;
#[path = "memory_push_options.rs"]
mod options;
#[path = "memory_pull.rs"]
mod pulled;

pub use listed::{agent_note_place, agent_script_place, features, lists};
use options::{Options, Reviewed, Selection};
use pulled::{pull, pull_scripts};

/// The id of the hidden team note.
const HIDDEN_ID: i64 = 40;

/// A note or script saved on this computer: local id, scope, repository, title or name, action,
/// conflict.
type LocalEntry = (
    i64,
    &'static str,
    Option<&'static str>,
    &'static str,
    &'static str,
    bool,
);

/// The notes saved on this computer.
const LOCAL: [LocalEntry; 7] = [
    (
        1,
        "repo",
        Some("github.com/o/r"),
        "Watcher misses renames",
        "add",
        false,
    ),
    (2, "global", None, "Prefer bun over npm", "identical", false),
    (3, "global", None, "Memory config lives in the config dir", "keep", true),
    (4, "repo", Some("github.com/a/b"), "Bump the cache key", "keep", true),
    (
        5,
        "repo",
        Some("github.com/o/r"),
        "retry the watcher on rename",
        "teamNewer",
        false,
    ),
    (6, "global", None, "Cache the tools probe", "update", false),
    (
        7,
        "repo",
        Some("github.com/a/b"),
        "A note the team refuses",
        "add",
        false,
    ),
];

/// The scripts saved on this computer.
const LOCAL_SCRIPTS: [LocalEntry; 7] = [
    (11, "repo", Some("github.com/o/r"), "open-prs", "add", false),
    (12, "global", None, "tidy-branches", "identical", false),
    (13, "global", None, "bump-cache", "keep", true),
    (14, "repo", Some("github.com/a/b"), "lint-all", "keep", true),
    (15, "repo", Some("github.com/o/r"), "watch-renames", "teamNewer", false),
    (16, "global", None, "probe-tools", "update", false),
    (17, "repo", Some("github.com/a/b"), "refused-script", "add", false),
];

/// What a push takes: its counts field, the field of a row's label, the entries, the one a real
/// run fails, and the variable that routes them (`None`: by note scope).
struct Pushed {
    field: &'static str,
    label: &'static str,
    entries: &'static [LocalEntry],
    failing: i64,
    routing: Option<&'static str>,
}

impl Pushed {
    /// How the scope `scope` of these entries is routed, by the variable of the entries.
    fn routed(&self, scope: &str) -> Option<String> {
        match self.routing {
            Some(var) => std::env::var(var).ok(),
            None => routing(scope),
        }
    }
}

/// `push`: the notes.
const NOTES: Pushed = Pushed {
    field: "notes",
    label: "title",
    entries: &LOCAL,
    failing: 7,
    routing: None,
};

/// `scripts push`: the scripts.
const SCRIPTS: Pushed = Pushed {
    field: "scripts",
    label: "name",
    entries: &LOCAL_SCRIPTS,
    failing: 17,
    routing: Some("HARNESS_MEMORY_SCRIPTS"),
};

/// `note` with `readOnly` and `hidden` (see the module docs).
pub fn routed(mut note: Value, hidden: bool) -> Value {
    let team_and_local = routing(note["scope"].as_str().unwrap_or_default()).as_deref() == Some("remote+local");

    note["readOnly"] = json!(team_and_local && note["place"] == "team");
    note["hidden"] = json!(hidden);
    note
}

/// How note scope `scope` is routed, by `HARNESS_MEMORY_REPOSITORY_NOTES` or
/// `HARNESS_MEMORY_GLOBAL_NOTES`.
fn routing(scope: &str) -> Option<String> {
    let var = if scope == "repo" {
        "HARNESS_MEMORY_REPOSITORY_NOTES"
    } else {
        "HARNESS_MEMORY_GLOBAL_NOTES"
    };

    std::env::var(var).ok()
}

/// Whether `note` is a read-only team note.
pub fn read_only(note: &Value) -> bool {
    routed(note.clone(), false)["readOnly"] == true
}

/// How many of `entries` are read-only team notes (a script has no `kind`).
pub fn kept(entries: &[&Value]) -> usize {
    entries
        .iter()
        .filter(|entry| entry.get("kind").is_some() && read_only(entry))
        .count()
}

/// The hidden team note, with its body.
pub fn hidden_fixtures() -> Vec<Value> {
    let note = json!({
        "id": HIDDEN_ID,
        "kind": "decision",
        "title": "A hidden team note",
        "scope": "global",
        "repo": null,
        "place": "team",
        "tags": ["hidden"],
        "entities": [],
        "author": { "source": "codex", "root": null },
        "createdAt": "2026-09-01T08:00:00Z",
        "updatedAt": "2026-09-02T08:00:00Z",
        "revision": 1,
        "hits": 0,
        "body": "Hidden on this computer.\n",
    });

    vec![routed(note, true)]
}

/// The fields a list adds while a team database is set.
pub fn list_extras(answer: &mut Value) {
    let local = |scope: &str| LOCAL.iter().filter(|note| note.1 == scope).count();

    answer["hiddenCount"] = json!(1);
    answer["localCounts"] = json!({ "repo": local("repo"), "global": local("global") });
}

/// `notes hide|unhide <id> --json [--project P]` on `notes`.
pub fn hide(command: &str, args: &[String], notes: &[Value]) -> ! {
    let id: i64 = args.first().and_then(|id| id.parse().ok()).unwrap_or_else(|| usage());
    let project = args
        .windows(2)
        .any(|pair| pair[0] == "--project" && !pair[1].is_empty());

    if !args.iter().any(|arg| arg == "--json") {
        usage();
    }

    let hidden = hidden_fixtures();
    let found = notes
        .iter()
        .chain(&hidden)
        .find(|note| note["id"] == id && (project || note["scope"] != "repo"));

    match found {
        None => fail(&format!("harness-memory: note {id} not found")),
        Some(note) if note["place"] != "team" => fail(&format!(
            "harness-memory: note {id} is saved on this computer, delete it instead"
        )),
        Some(_) => {
            let field = if command == "hide" { "hidden" } else { "unhidden" };
            println!("{}", json!({ field: id }));
            std::process::exit(0);
        }
    }
}

/// `notes discard-local (--all | - | IDS) [--scope S] --yes --json`.
pub fn discard_local(args: &[String]) -> ! {
    let options = Options::parse(args, &["--yes", "--json"]);

    if !options.flags.iter().any(|flag| flag == "--yes") {
        usage();
    }

    let deleted = match options.which.as_deref() {
        Some("--all") => LOCAL.iter().filter(|note| options.in_scope(note.1)).count(),
        Some(ids) => ids_of(ids).len(),
        None => usage(),
    };

    println!("{}", json!({ "deleted": deleted }));
    std::process::exit(0);
}

/// `push …` or `pull …` (`args` start with the command; see the module docs).
pub fn copy(args: &[String]) -> ! {
    match args {
        [command, rest @ ..] if command == "pull" => pull(rest),
        [_, rest @ ..] => push_of(&NOTES, rest),
        [] => usage(),
    }
}

/// Whether `args` (starting with `scripts`) are `scripts push …` or `scripts pull …`.
pub fn copies(args: &[String]) -> bool {
    args.get(1)
        .is_some_and(|command| command == "push" || command == "pull")
}

/// `scripts push …` or `scripts pull …` (`args` start with `scripts`).
pub fn copy_scripts(args: &[String]) -> ! {
    match args {
        [_, command, rest @ ..] if command == "pull" => pull_scripts(rest),
        [_, _, rest @ ..] => push_of(&SCRIPTS, rest),
        _ => usage(),
    }
}

fn push_of(pushed: &Pushed, args: &[String]) -> ! {
    let options = Options::parse(args, &["--dry-run", "--json", "--delete-copied", "--remote-local"]);
    let selection = options.selection.as_deref();
    let flag = |name: &str| options.flags.iter().any(|flag| flag == name);
    let dry_run = flag("--dry-run");

    let unlisted =
        (flag("--remote-local") && !lists("push-remote-local")) || (selection.is_some() && !lists("push-selection"));
    let both = selection.is_some() && options.overwrite.is_some();

    if !flag("--json")
        || (dry_run && flag("--delete-copied"))
        || options.which.is_some()
        || unlisted
        || both
        || selection.is_some_and(|source| source != "-")
    {
        usage();
    }

    let mode = std::env::var("FAKE_MEMORY_PUSH").unwrap_or_default();

    if mode == "usage" {
        usage();
    }

    if mode == "nothing" {
        let nothing = format!("no {} on this computer", pushed.field);
        println!(
            "{}",
            json!({ "dryRun": dry_run, "nothing": nothing, "changes": [], "failed": [] })
        );
        std::process::exit(0);
    }

    let chosen = selection.map(|_| Selection::read());
    let picked = match (&chosen, options.overwrite.as_deref()) {
        (Some(chosen), _) => chosen.overwrite.clone(),
        (None, None | Some("none")) => Vec::new(),
        (None, Some("all")) => pushed.entries.iter().map(|entry| entry.0).collect(),
        (None, Some(ids)) => ids_of(ids),
    };
    let first = pushed.entries[0].0;
    // What the entry is now: a real run sees the first entry saved again after the review.
    let updated_at = |id: i64| {
        if !dry_run && mode == "stale" && id == first {
            return format!("2026-10-12T09:00:{id:02}.750000Z");
        }

        if mode == "undated" && id == first {
            return String::new();
        }

        format!("2026-10-10T10:00:{id:02}.250000Z")
    };
    let label_of = |id: i64| {
        let label = pushed
            .entries
            .iter()
            .find(|entry| entry.0 == id)
            .map_or("", |entry| entry.3);

        if !dry_run && mode == "renamed" && id == first {
            format!("{label} (edited)")
        } else {
            label.to_owned()
        }
    };
    let reviewed = |id: i64| {
        chosen
            .as_ref()
            .and_then(|chosen| chosen.only.iter().find(|entry| entry.id == id))
    };
    let named = |id: i64| chosen.is_none() || reviewed(id).is_some();
    let current = |entry: &Reviewed| entry.name == label_of(entry.id) && entry.updated_at == updated_at(entry.id);
    let stale = |id: i64| reviewed(id).is_some_and(|entry| !current(entry));
    let late = |id: i64| dry_run && mode == "late" && id == pushed.entries[5].0;

    let hidden = |id: i64| mode == "hidden" && id == pushed.entries[3].0;
    let fails = |id: i64| (!dry_run && id == pushed.failing) || hidden(id) || stale(id);
    let relation_fails = !dry_run && mode == "relation";
    let undeleted = |id: i64| !dry_run && mode == "undeleted" && flag("--delete-copied") && id == pushed.entries[0].0;
    let changed = |id: i64| !dry_run && mode == "changed" && flag("--delete-copied") && id == pushed.entries[1].0;
    let changes: Vec<Value> = pushed
        .entries
        .iter()
        .filter(|entry| options.pushed(pushed, entry.1) && named(entry.0) && !late(entry.0))
        .map(|&(id, scope, repo, _, action, conflict)| {
            let action = if conflict && picked.contains(&id) {
                "overwrite"
            } else {
                action
            };
            let copied = ["add", "update", "overwrite", "identical"].contains(&action);

            json!({
                "localId": id, "scope": scope, "repo": repo, pushed.label: label_of(id), "action": action,
                "conflict": conflict, "teamId": if action == "add" { Value::Null } else { json!(id + 30) },
                "localUpdatedAt": if updated_at(id).is_empty() { Value::Null } else { json!(updated_at(id)) },
                "teamUpdatedAt": if action == "add" { Value::Null } else { json!("2026-10-09T10:00:00Z") },
                "deleted": !dry_run && !relation_fails && flag("--delete-copied") && copied && !fails(id) && !undeleted(id) && !changed(id),
                "failed": fails(id),
                "changedSince": changed(id),
            })
        })
        .map(|mut change: Value| {
            if mode == "unflagged" {
                change.as_object_mut().map(|fields| fields.remove("failed"));
            }

            change
        })
        .collect();

    let mut failed: Vec<Value> = changes
        .iter()
        .filter_map(|change| change["localId"].as_i64())
        .filter(|id| fails(*id))
        .map(|id| {
            let message = if stale(id) {
                "changed since the review, so it was not copied".to_owned()
            } else if hidden(id) {
                format!("a team entry hidden on this computer matches it in postgres://team:hunter2@db.example.com/memory, unhide {} to copy it", id + 30)
            } else {
                "cannot write to postgres://team:hunter2@db.example.com/memory".to_owned()
            };

            json!({ "localId": id, "message": message })
        })
        .collect();

    failed.extend(
        pushed
            .entries
            .iter()
            .filter(|entry| undeleted(entry.0))
            .map(|entry| json!({ "localId": entry.0, "message": "copied, but the local copy could not be deleted" })),
    );

    if relation_fails {
        failed.push(json!({
            "localId": null,
            "message": "a relation was not pushed to postgres://team:hunter2@db.example.com/memory, nothing deleted, push again",
        }));
    }

    let deleted = changes.iter().filter(|change| change["deleted"] == true).count();
    let added = changes.iter().filter(|change| change["action"] == "add").count();

    println!(
        "{}",
        json!({
            "dryRun": dry_run, "nothing": null, "changes": changes, "failed": failed,
            pushed.field: { "added": added },
            "deleted": if dry_run { Value::Null } else { json!(deleted) },
        })
    );
    std::process::exit(if failed.is_empty() { 0 } else { 1 });
}

/// The ids of `ids` (comma-separated), or of stdin for `-`.
fn ids_of(ids: &str) -> Vec<i64> {
    let mut text = ids.to_owned();

    if ids == "-" {
        text.clear();
        std::io::stdin().read_to_string(&mut text).unwrap_or_else(|_| usage());
    }

    text.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|id| !id.is_empty())
        .map(|id| id.parse().unwrap_or_else(|_| usage()))
        .collect()
}

fn fail(line: &str) -> ! {
    eprintln!("{line}");
    std::process::exit(1);
}

fn usage() -> ! {
    eprintln!("usage: harness-memory [features | push --json | scripts push --json | notes hide|unhide|discard-local]");
    std::process::exit(2);
}

/// With `FAKE_MEMORY_OFFLINE` set (any value), makes list `answer` the one of a server whose team
/// database is unreachable: no `field` entries, `"total": 0` and `"offline": true`.
pub fn offline(answer: &mut Value, field: &str) {
    if std::env::var_os("FAKE_MEMORY_OFFLINE").is_some() {
        answer[field] = json!([]);
        answer["total"] = json!(0);
        answer["offline"] = json!(true);
    }
}
