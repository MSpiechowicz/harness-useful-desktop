//! `harness-memory notes …` and `harness-memory scripts list|delete …` stand-ins, answered from fixed
//! fixtures (nothing is kept between runs, so a deleted note comes back).
//!
//! - `notes list --json --offset N --limit N [--query Q] [--kind K] --sort
//!   updated|title|kind|scope|place --order asc|desc [--project P]` prints `{"notes", "total", "kinds"}`: the global notes, plus the
//!   repository ones with `--project`, one of them of a kind the app does not know (`idea`).
//!   `--query` matches titles (any case); `kinds` counts every kind under the query.
//! - `notes get <id> --json [--project P]` prints `{"note", "links"}`; `notes delete <id>
//!   [--project P]` prints `deleted note <id> <title>`. A note not visible from the arguments exits
//!   1 with `harness-memory: note <id> not found`; id [`FAILING_ID`] exits 1 with a URL with a
//!   password on stderr.
//! - `notes delete --all --yes --json [--project P]` and `scripts delete --all --yes --json
//!   [--project P]` print `{"deleted", "team"}`: how many fixtures are visible from the arguments,
//!   and how many of those are kept in the team database (`place` `team`).
//! - `scripts list --json` takes the same paging options, with `--group needsReview|repo|global`
//!   instead of `--kind` and `--sort updated|name|language|scope|place|runs|lastRun` (a script
//!   never run last either way); it prints `{"scripts", "total", "groups"}` from the stand-in's
//!   scripts (see [`crate::memory`]).
//! - An option this stand-in does not know, a missing value, or `FAKE_MEMORY_CLI=unpaged` (a
//!   server whose commands take no pages) exits 2 with a usage line, as the real server does.

use std::cmp::Ordering;

use serde_json::{Value, json};

/// The id whose `notes get` and `notes delete` fail as a broken team database would.
const FAILING_ID: i64 = 500;

/// What `--project` adds: the repository the stand-in's repository notes belong to.
const REPO: &str = "github.com/o/r";

/// The options of one command.
#[derive(Default)]
struct Options {
    json: bool,
    offset: usize,
    limit: Option<usize>,
    query: Option<String>,
    filter: Option<String>,
    sort: Option<String>,
    order: Option<String>,
    project: bool,
    id: Option<i64>,
}

/// `harness-memory notes <args>`.
pub fn notes(args: &[String]) -> ! {
    match args.first().map(String::as_str) {
        Some("list") => {
            let options = parse(&args[1..], "--kind", false);
            list("notes", without_bodies(note_fixtures()), &options, &kind_counts)
        }
        Some("delete") if args.get(1).is_some_and(|arg| arg == "--all") => delete_all(&args[1..], note_fixtures()),
        Some(command @ ("get" | "delete")) => {
            let options = parse(&args[1..], "", true);
            one_note(command, &options)
        }
        _ => usage(),
    }
}

/// `harness-memory scripts <args>`.
pub fn scripts(args: &[String]) -> ! {
    match args.first().map(String::as_str) {
        Some("list") => {}
        Some("delete") => delete_all(&args[1..], crate::memory::fixtures()),
        _ => usage(),
    }

    let options = parse(&args[1..], "--group", false);
    list(
        "scripts",
        without_bodies(crate::memory::fixtures()),
        &options,
        &group_counts,
    )
}

/// `delete --all --yes --json [--project P]` on `entries` (see the module docs); exits 0.
fn delete_all(args: &[String], entries: Vec<Value>) -> ! {
    let mut flags: Vec<&str> = Vec::new();
    let mut project = false;
    let mut rest = args.iter();

    while let Some(arg) = rest.next() {
        match arg.as_str() {
            flag @ ("--all" | "--yes" | "--json") => flags.push(flag),
            "--project" => project = !rest.next().cloned().unwrap_or_else(|| usage()).is_empty(),
            _ => usage(),
        }
    }

    let unpaged = std::env::var("FAKE_MEMORY_CLI").is_ok_and(|mode| mode == "unpaged");

    if unpaged || flags != ["--all", "--yes", "--json"] {
        usage();
    }

    let visible: Vec<&Value> = entries
        .iter()
        .filter(|entry| project || entry["scope"] != "repo")
        .collect();
    let team = visible.iter().filter(|entry| entry["place"] == "team").count();

    println!("{}", json!({ "deleted": visible.len(), "team": team }));
    std::process::exit(0);
}

/// `entries` as a list shows them: without `body`.
fn without_bodies(entries: Vec<Value>) -> Vec<Value> {
    entries
        .into_iter()
        .map(|mut entry| {
            entry.as_object_mut().map(|fields| fields.remove("body"));
            entry
        })
        .collect()
}

fn usage() -> ! {
    eprintln!("usage: harness-memory [notes list --json | scripts list --json] [--offset N --limit N]");
    std::process::exit(2);
}

/// The options in `args`; `filter` is the command's own option (`--kind`), `with_id` takes a
/// leading id.
fn parse(args: &[String], filter: &str, with_id: bool) -> Options {
    if std::env::var("FAKE_MEMORY_CLI").is_ok_and(|mode| mode == "unpaged") {
        usage();
    }

    let mut options = Options::default();
    let mut rest = args.iter();

    if with_id {
        options.id = Some(rest.next().and_then(|id| id.parse().ok()).unwrap_or_else(|| usage()));
    }

    while let Some(arg) = rest.next() {
        if arg == "--json" {
            options.json = true;
            continue;
        }

        let mut value = || rest.next().cloned().unwrap_or_else(|| usage());

        match arg.as_str() {
            "--offset" => options.offset = value().parse().unwrap_or_else(|_| usage()),
            "--limit" => options.limit = Some(value().parse().unwrap_or_else(|_| usage())),
            "--query" => options.query = Some(value()),
            "--sort" => options.sort = Some(value()),
            "--order" => options.order = Some(value()),
            "--project" => options.project = !value().is_empty(),
            other if !filter.is_empty() && other == filter => options.filter = Some(value()),
            _ => usage(),
        }
    }

    options
}

/// Prints one page of `entries` as `{field, total, <counts>}` and exits 0.
fn list(field: &str, entries: Vec<Value>, options: &Options, counts: &dyn Fn(&[Value]) -> (&'static str, Value)) -> ! {
    if !options.json || options.limit.is_none() {
        usage();
    }

    let query = options.query.as_deref().unwrap_or_default().to_lowercase();
    let name_field = if field == "notes" { "title" } else { "name" };

    let mut matching: Vec<Value> = entries
        .into_iter()
        .filter(|entry| options.project || entry["scope"] != "repo")
        .filter(|entry| {
            let name = entry[name_field].as_str().unwrap_or_default().to_lowercase();
            name.contains(&query)
        })
        .collect();

    let (counts_field, counted) = counts(&matching);

    if let Some(filter) = &options.filter {
        matching.retain(|entry| in_group(entry, filter));
    }

    let key = match options.sort.as_deref() {
        Some("updated") => "updatedAt",
        Some("title" | "name") => name_field,
        Some(sort @ ("kind" | "scope" | "place" | "language")) => sort,
        Some("runs") => "runCount",
        Some("lastRun") => "lastRunAt",
        _ => usage(),
    };
    matching.sort_by(|a, b| compare(&a[key], &b[key]));

    match options.order.as_deref() {
        Some("asc") => {}
        Some("desc") => matching.reverse(),
        _ => usage(),
    }

    // A script never run comes last whichever the order, as the real server lists it.
    matching.sort_by_key(|entry| entry[key].is_null());

    let total = matching.len();
    let page: Vec<Value> = matching
        .into_iter()
        .skip(options.offset)
        .take(options.limit.unwrap_or_default())
        .collect();

    println!("{}", json!({ field: page, "total": total, counts_field: counted }));
    std::process::exit(0);
}

/// Strings by their text, numbers by their value.
fn compare(a: &Value, b: &Value) -> Ordering {
    match (a.as_i64(), b.as_i64()) {
        (Some(a), Some(b)) => a.cmp(&b),
        _ => a.as_str().unwrap_or_default().cmp(b.as_str().unwrap_or_default()),
    }
}

/// Whether `entry` is of note kind or script group `filter`.
fn in_group(entry: &Value, filter: &str) -> bool {
    match filter {
        "needsReview" => entry["trusted"] == false,
        "repo" | "global" => entry["scope"] == filter,
        kind => entry["kind"] == kind,
    }
}

fn kind_counts(notes: &[Value]) -> (&'static str, Value) {
    let mut kinds: Vec<Value> = Vec::new();

    for note in notes {
        match kinds.iter_mut().find(|entry| entry["kind"] == note["kind"]) {
            Some(entry) => entry["count"] = json!(entry["count"].as_u64().unwrap_or_default() + 1),
            None => kinds.push(json!({ "kind": note["kind"], "count": 1 })),
        }
    }

    ("kinds", json!(kinds))
}

fn group_counts(scripts: &[Value]) -> (&'static str, Value) {
    let count = |group: &str| scripts.iter().filter(|script| in_group(script, group)).count();

    (
        "groups",
        json!({ "needsReview": count("needsReview"), "repo": count("repo"), "global": count("global") }),
    )
}

/// `notes get` and `notes delete` (see the module docs).
fn one_note(command: &str, options: &Options) -> ! {
    let id = options.id.unwrap_or_default();

    if id == FAILING_ID {
        eprintln!("warning: slow store");
        eprintln!(
            "harness-memory: the team database cannot be reached (postgres://team:hunter2@db.example.com/memory)"
        );
        std::process::exit(1);
    }

    let notes = note_fixtures();
    let visible = |note: &&Value| note["id"] == id && (options.project || note["scope"] != "repo");

    let Some(note) = notes.iter().find(visible) else {
        eprintln!("harness-memory: note {id} not found");
        std::process::exit(1);
    };

    if command == "delete" {
        println!("deleted note {id} {}", note["title"].as_str().unwrap_or_default());
        std::process::exit(0);
    }

    if !options.json {
        usage();
    }

    let links: Vec<Value> = LINKS
        .iter()
        .filter_map(|(from, to, relation)| match (*from == id, *to == id) {
            (true, _) => link(&notes, *to, relation, "out"),
            (_, true) => link(&notes, *from, relation, "in"),
            _ => None,
        })
        .collect();

    println!("{}", json!({ "note": note, "links": links }));
    std::process::exit(0);
}

/// The links between fixtures: from, to, relation.
const LINKS: [(i64, i64, &str); 3] = [(2, 3, "fixes"), (5, 2, "follows"), (6, 1, "explains")];

fn link(notes: &[Value], other: i64, relation: &str, direction: &str) -> Option<Value> {
    let note = notes.iter().find(|note| note["id"] == other)?;

    Some(json!({
        "id": other,
        "title": note["title"],
        "kind": note["kind"],
        "relation": relation,
        "direction": direction,
    }))
}

fn note_fixtures() -> Vec<Value> {
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

    vec![
        note(
            1,
            "decision",
            "Use rustls everywhere",
            "global",
            "team",
            "2026-10-09T10:00:00Z",
        ),
        note(
            2,
            "fix",
            "Retry the watcher on rename",
            "repo",
            "team",
            "2026-10-08T10:00:00Z",
        ),
        note(
            3,
            "problem",
            "Watcher misses renames",
            "repo",
            "local",
            "2026-10-07T10:00:00Z",
        ),
        note(
            4,
            "outcome",
            "Release 0.19 shipped",
            "global",
            "team",
            "2026-10-06T10:00:00Z",
        ),
        note(
            5,
            "optimisation",
            "Cache the tools probe",
            "repo",
            "local",
            "2026-10-05T10:00:00Z",
        ),
        note(
            6,
            "location",
            "Memory config lives in the config dir",
            "global",
            "local",
            "2026-10-04T10:00:00Z",
        ),
        note(
            7,
            "note",
            "Prefer bun over npm",
            "global",
            "local",
            "2026-10-03T10:00:00Z",
        ),
        note(
            8,
            "idea",
            "A kind from a newer server",
            "global",
            "local",
            "2026-10-02T10:00:00Z",
        ),
    ]
}
