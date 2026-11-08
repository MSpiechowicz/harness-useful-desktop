//! `harness-memory pull --json` and `harness-memory scripts pull --json`, the copy of the team's
//! entries of the scopes routed `remote+local` to this computer, answered from fixed fixtures
//! (nothing is kept between runs). A child module of `memory_push`, whose routing it shares.
//!
//! - `pull --dry-run --json` answers the [`TEAM_NOTES`] of the scopes routed `remote+local` (by
//!   `HARNESS_MEMORY_REPOSITORY_NOTES` and `HARNESS_MEMORY_GLOBAL_NOTES`; scripts by
//!   `HARNESS_MEMORY_SCRIPTS`): `add` for one that is not here, `identical` and `localDiffers` for
//!   one that is. With no such scope it has `nothing` to copy.
//! - `pull --json --ids -` reads the team ids to copy on stdin (one per line) and answers those
//!   entries, the failing one marked `"failed": true` with an entry of `failed` (exit 1, a URL with
//!   a password in its message). Without `--ids` a real run is a usage error, as is `--ids` with
//!   `--dry-run`.
//! - `scripts pull` answers [`TEAM_SCRIPTS`] the same way (a `name` for a `title`, a `scripts`
//!   counts object for `notes`).
//!
//! It is a usage error (exit 2) unless the stand-in lists `pull-json` (`scripts-pull-json` for
//! scripts). `FAKE_MEMORY_PUSH=nothing` has nothing to copy, `FAKE_MEMORY_PUSH=usage` predates
//! `pull --json` (exit 2), and `FAKE_MEMORY_PUSH=hidden` fails the failing entry in a dry run too.

use serde_json::{Value, json};

use super::{NOTES, Pushed, SCRIPTS, ids_of, lists, usage};

/// A team entry: team id, id of the copy here when there is one, scope, repository, title or name,
/// action.
type TeamEntry = (
    i64,
    Option<i64>,
    &'static str,
    Option<&'static str>,
    &'static str,
    &'static str,
);

/// The team's notes.
const TEAM_NOTES: [TeamEntry; 6] = [
    (
        41,
        None,
        "repo",
        Some("github.com/o/r"),
        "Watcher misses renames upstream",
        "add",
    ),
    (42, None, "global", None, "Prefer pnpm in CI", "add"),
    (
        43,
        None,
        "repo",
        Some("github.com/a/b"),
        "A note this computer refuses",
        "add",
    ),
    (44, Some(2), "global", None, "Prefer bun over npm", "identical"),
    (
        45,
        Some(3),
        "global",
        None,
        "Memory config lives in the config dir",
        "localDiffers",
    ),
    (
        46,
        None,
        "repo",
        Some("github.com/a/b"),
        "Bump the cache key twice",
        "add",
    ),
];

/// The team's scripts.
const TEAM_SCRIPTS: [TeamEntry; 6] = [
    (51, None, "repo", Some("github.com/o/r"), "list-prs", "add"),
    (52, None, "global", None, "prune-branches", "add"),
    (53, None, "repo", Some("github.com/a/b"), "refused-pull", "add"),
    (54, Some(12), "global", None, "tidy-branches", "identical"),
    (55, Some(13), "global", None, "bump-cache", "localDiffers"),
    (56, None, "repo", Some("github.com/a/b"), "lint-twice", "add"),
];

/// What a pull takes: the entries on the team side, the id of the one a real run fails, the
/// feature that lists it and the push whose names and routing it shares.
struct Pulled {
    entries: &'static [TeamEntry],
    failing: i64,
    feature: &'static str,
    pushed: &'static Pushed,
}

const PULL_NOTES: Pulled = Pulled {
    entries: &TEAM_NOTES,
    failing: 43,
    feature: "pull-json",
    pushed: &NOTES,
};

const PULL_SCRIPTS: Pulled = Pulled {
    entries: &TEAM_SCRIPTS,
    failing: 53,
    feature: "scripts-pull-json",
    pushed: &SCRIPTS,
};

/// `pull --json …` (see the module docs).
pub fn pull(args: &[String]) -> ! {
    pull_of(&PULL_NOTES, args)
}

/// `scripts pull --json …` (see the module docs).
pub fn pull_scripts(args: &[String]) -> ! {
    pull_of(&PULL_SCRIPTS, args)
}

fn pull_of(pulled: &Pulled, args: &[String]) -> ! {
    let (dry_run, ids) = options(args);
    let mode = std::env::var("FAKE_MEMORY_PUSH").unwrap_or_default();

    if mode == "usage" || !lists(pulled.feature) || dry_run == ids.is_some() {
        usage();
    }

    let kept_here = |scope: &str| pulled.pushed.routed(scope).as_deref() == Some("remote+local");
    let in_scope: Vec<&TeamEntry> = pulled.entries.iter().filter(|entry| kept_here(entry.2)).collect();

    if mode == "nothing" || in_scope.is_empty() {
        let nothing = format!("no {} are kept on this computer too", pulled.pushed.field);
        println!(
            "{}",
            json!({ "dryRun": dry_run, "nothing": nothing, "changes": [], "failed": [] })
        );
        std::process::exit(0);
    }

    let picked = ids.map(|ids| ids_of(&ids));
    let taken: Vec<&&TeamEntry> = in_scope
        .iter()
        .filter(|entry| picked.as_ref().is_none_or(|ids| ids.contains(&entry.0)))
        .collect();

    let fails = |id: i64| id == pulled.failing && (!dry_run || mode == "hidden");

    let changes: Vec<Value> = taken
        .iter()
        .map(|&&&(team_id, local_id, scope, repo, label, action)| {
            json!({
                "teamId": team_id, "localId": local_id, "scope": scope, "repo": repo,
                pulled.pushed.label: label, "action": action,
                "teamUpdatedAt": "2026-10-11T10:00:00Z",
                "localUpdatedAt": local_id.map(|_| "2026-10-10T10:00:00Z"),
                "failed": action == "add" && fails(team_id),
            })
        })
        .collect();

    let failed: Vec<Value> = changes
        .iter()
        .filter(|change| change["failed"] == true)
        .map(|change| {
            json!({
                "teamId": change["teamId"],
                "message": "cannot write to postgres://team:hunter2@db.example.com/memory",
            })
        })
        .collect();

    let count = |action: &str| changes.iter().filter(|change| change["action"] == action).count();

    println!(
        "{}",
        json!({
            "dryRun": dry_run, "nothing": null, "changes": changes, "failed": failed,
            pulled.pushed.field: {
                "added": count("add"), "identical": count("identical"),
                "localDiffers": count("localDiffers"), "failed": failed.len(),
            },
        })
    );
    std::process::exit(if failed.is_empty() { 0 } else { 1 });
}

/// Whether `--dry-run` is given, and the value of `--ids`; anything but `--json` and those is a
/// usage error.
fn options(args: &[String]) -> (bool, Option<String>) {
    let (mut dry_run, mut json, mut ids) = (false, false, None);
    let mut rest = args.iter();

    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--json" => json = true,
            "--ids" => ids = Some(rest.next().cloned().unwrap_or_else(|| usage())),
            _ => usage(),
        }
    }

    if !json || ids.as_deref().is_some_and(|ids| ids != "-") {
        usage();
    }

    (dry_run, ids)
}
