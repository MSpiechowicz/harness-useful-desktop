//! `harness-memory notes …` and `harness-memory scripts list|delete …` stand-ins, answered from fixed
//! fixtures (nothing is kept between runs, so a deleted note comes back). `scripts get|hide|unhide|
//! discard-local`, `scripts list --hidden` and the team fields of scripts are
//! [`crate::memory_scripts_hidden`]'s.
//!
//! - `notes list --json --offset N --limit N [--query Q] [--kind K] [--scope repo|global] [--place
//!   local|team] --sort updated|title|kind|scope|place|useful|lastUseful --order asc|desc
//!   [--project P]` prints
//!   `{"notes", "total", "kinds", "scopes", "places"}`: the global notes, plus the repository ones
//!   with `--project`, one of them of a kind the app does not know (`idea`). `--query` matches
//!   titles (any case); `kinds` counts every kind under the query, scope and place, `scopes` both
//!   scopes under the query, kind and place, `places` both places under every other filter.
//!   `--lang en|other|unknown` keeps the notes whose `lang` is that (a fixture has none, so only
//!   `unknown` keeps it) and is a usage error unless the stand-in lists `notes-lang`.
//!   With `notes-useful` listed the notes carry `shown`, `useful` and `lastUsefulAt` (see
//!   [`crate::memory_cli_notes`]), `--useful ever|never` keeps those used at least once or shown at least three
//!   times and never used, and `--sort useful|lastUseful` orders by them (a note never used comes
//!   last in `lastUseful`). Without it the notes carry none and the option and the sorts are usage
//!   errors (`FAKE_MEMORY_FEATURES=notes-lang,...` is an older server).
//! - `notes get <id> --json [--project P]` prints `{"note", "links"}`; `notes delete <id>
//!   [--project P]` prints `deleted note <id> <title>`. A note not visible from the arguments exits
//!   1 with `harness-memory: note <id> not found`; id [`FAILING_ID`] exits 1 with a URL with a
//!   password on stderr.
//! - `notes delete --all --yes --json [--project P]` and `scripts delete --all --yes --json
//!   [--project P]` print `{"deleted", "team"}`: how many fixtures are visible from the arguments,
//!   and how many of those are kept in the team database (`place` `team`); read-only team notes
//!   and scripts stay (`"kept"`, when there are any).
//! - `scripts list --json` takes the same paging options, with `--group needsReview|repo|global`
//!   instead of `--kind`, or `--scope repo|global` and `--review needed|trusted` (not with
//!   `--group`), `--place local|team`, and `--sort updated|name|language|scope|place|runs|lastRun`
//!   (a script never run last either way); it prints `{"scripts", "total", "groups", "scopes",
//!   "review", "places"}` from the stand-in's scripts (see [`crate::memory`]): `groups` under the
//!   query and place, `scopes` under the query, review and place, `review` under the query, scope
//!   and place, `places` under every other filter.
//! - An option this stand-in does not know, a missing value, or `FAKE_MEMORY_CLI=unpaged` (a
//!   server whose commands take no pages) exits 2 with a usage line, as the real server does.
//!   `FAKE_MEMORY_CLI=single` is a server that predates combined filters: `--scope` and `--review`
//!   are usage errors, and lists answer without `scopes` and `review`. `FAKE_MEMORY_CLI=scoped` is
//!   one that combines filters but predates the place: `--place` is a usage error, and lists
//!   answer without `places`; `single` predates the place too.
//! - `notes list|get --hidden`, `notes hide|unhide|discard-local` and the team-and-local fields
//!   of the notes are [`crate::memory_push`]'s.
//! - `FAKE_MEMORY_NOTES` adds a global note for each of its lines (that title), from id 100 on, to
//!   the notes of these commands and of the MCP stand-in.

use std::cmp::Ordering;

use serde_json::{Value, json};

use crate::memory_cli_notes::{LINKS, link, note_fixtures, with_usefulness};

/// The id whose `notes get` and `notes delete` fail as a broken team database would.
const FAILING_ID: i64 = 500;

/// The options of one command.
#[derive(Default)]
struct Options {
    json: bool,
    offset: usize,
    limit: Option<usize>,
    query: Option<String>,
    filter: Option<String>,
    scope: Option<String>,
    review: Option<String>,
    /// Whether this server combines filters (not `FAKE_MEMORY_CLI=single`).
    combines: bool,
    place: Option<String>,
    /// Whether this server filters by place (neither `single` nor `scoped`).
    places: bool,
    /// `--lang`, taken only when the stand-in lists `notes-lang`.
    lang: Option<String>,
    /// `--useful`, taken only when the stand-in lists `notes-useful`.
    useful: Option<String>,
    sort: Option<String>,
    order: Option<String>,
    project: bool,
    id: Option<i64>,
    hidden: bool,
}

/// `harness-memory notes <args>`.
pub fn notes(args: &[String]) -> ! {
    match args.first().map(String::as_str) {
        Some("list") => {
            let options = parse(&args[1..], "--kind", false);
            list("notes", without_bodies(visible_notes(&options)), &options)
        }
        Some(command @ ("hide" | "unhide")) => crate::memory_push::hide(command, &args[1..], &note_fixtures()),
        Some("discard-local") => crate::memory_push::discard_local(&args[1..]),
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
        Some(command) => crate::memory_scripts_hidden::command(command, &args[1..]),
        None => usage(),
    }

    let options = parse(&args[1..], "--group", false);

    if options.filter.is_some() && (options.scope.is_some() || options.review.is_some()) {
        usage();
    }

    let scripts = if options.hidden {
        crate::memory_scripts_hidden::hidden_scripts()
    } else {
        crate::memory::fixtures()
    };

    list("scripts", without_bodies(scripts), &options)
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
    let kept = crate::memory_push::kept(&visible) + crate::memory_scripts_hidden::read_only_count(&visible);
    let mut answer = json!({ "deleted": visible.len() - kept, "team": team - kept });

    if kept > 0 {
        answer["kept"] = json!(kept);
    }

    println!("{answer}");
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
    let mode = std::env::var("FAKE_MEMORY_CLI").unwrap_or_default();

    if mode == "unpaged" {
        usage();
    }

    let mut options = Options {
        combines: mode != "single",
        places: mode != "single" && mode != "scoped",
        ..Options::default()
    };
    let mut rest = args.iter();

    if with_id {
        options.id = Some(rest.next().and_then(|id| id.parse().ok()).unwrap_or_else(|| usage()));
    }

    while let Some(arg) = rest.next() {
        if arg == "--json" || arg == "--hidden" {
            options.json |= arg == "--json";
            options.hidden |= arg == "--hidden";
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
            "--scope" if options.combines => options.scope = Some(value()),
            "--place" if options.places => options.place = Some(value()),
            "--lang" if filter == "--kind" && crate::memory_push::lists("notes-lang") => options.lang = Some(value()),
            "--useful" if filter == "--kind" && crate::memory_push::lists("notes-useful") => {
                options.useful = Some(value()).filter(|useful| matches!(useful.as_str(), "ever" | "never"));

                if options.useful.is_none() {
                    usage();
                }
            }
            "--review" if options.combines && filter == "--group" => options.review = Some(value()),
            other if !filter.is_empty() && other == filter => options.filter = Some(value()),
            _ => usage(),
        }
    }

    options
}

/// Prints one page of `entries` as `{field, total, <counts>}` and exits 0 (see the module docs).
fn list(field: &str, entries: Vec<Value>, options: &Options) -> ! {
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

    let notes = field == "notes";
    let keeps_filter = |entry: &Value| options.filter.as_ref().is_none_or(|filter| in_group(entry, filter));
    let keeps_scope = |entry: &Value| {
        options
            .scope
            .as_ref()
            .is_none_or(|scope| entry["scope"] == scope.as_str())
    };
    let keeps_review = |entry: &Value| options.review.as_ref().is_none_or(|review| review_of(entry) == review);
    let keeps_lang = |entry: &Value| {
        options.lang.as_ref().is_none_or(|lang| match entry["lang"].as_str() {
            Some(found) => found == lang,
            None => lang == "unknown",
        })
    };
    let keeps_useful = |entry: &Value| {
        options.useful.as_ref().is_none_or(|useful| {
            let (shown, used) = (
                entry["shown"].as_u64().unwrap_or(0),
                entry["useful"].as_u64().unwrap_or(0),
            );

            match useful.as_str() {
                "ever" => used >= 1,
                _ => shown >= 3 && used == 0,
            }
        })
    };
    let keeps_place = |entry: &Value| {
        options
            .place
            .as_ref()
            .is_none_or(|place| entry["place"] == place.as_str())
    };

    let in_other_filters: Vec<&Value> = matching
        .iter()
        .filter(|entry| keeps_filter(entry) && keeps_scope(entry) && keeps_review(entry))
        .collect();
    let place = |name: &str| in_other_filters.iter().filter(|entry| entry["place"] == name).count();
    let places = json!({ "local": place("local"), "team": place("team") });

    matching.retain(|entry| keeps_place(entry) && keeps_lang(entry) && keeps_useful(entry));

    let counts = if notes {
        let in_scope: Vec<Value> = matching.iter().filter(|entry| keeps_scope(entry)).cloned().collect();
        let of_kind: Vec<&Value> = matching.iter().filter(|entry| keeps_filter(entry)).collect();

        json!({ "kinds": kind_counts(&in_scope), "scopes": scope_counts(&of_kind) })
    } else {
        let reviewed: Vec<&Value> = matching.iter().filter(|entry| keeps_review(entry)).collect();
        let in_scope: Vec<&Value> = matching.iter().filter(|entry| keeps_scope(entry)).collect();
        let review = |name: &str| in_scope.iter().filter(|entry| review_of(entry) == name).count();

        json!({
            "groups": group_counts(&matching),
            "scopes": scope_counts(&reviewed),
            "review": { "needed": review("needed"), "trusted": review("trusted") },
        })
    };

    matching.retain(|entry| keeps_filter(entry) && keeps_scope(entry) && keeps_review(entry));

    let key = match options.sort.as_deref() {
        Some("updated") => "updatedAt",
        Some("title" | "name") => name_field,
        Some(sort @ ("kind" | "scope" | "place" | "language")) => sort,
        Some("runs") => "runCount",
        Some("lastRun") => "lastRunAt",
        Some("useful") if notes && crate::memory_push::lists("notes-useful") => "useful",
        Some("lastUseful") if notes && crate::memory_push::lists("notes-useful") => "lastUsefulAt",
        _ => usage(),
    };
    matching.sort_by(|a, b| compare(&a[key], &b[key]));

    match options.order.as_deref() {
        Some("asc") => {}
        Some("desc") => matching.reverse(),
        _ => usage(),
    }

    // A script never run (or a note never used, for `lastUseful`) comes last whichever the order,
    // as the real server lists it.
    matching.sort_by_key(|entry| entry[key].is_null());

    let total = matching.len();
    let page: Vec<Value> = matching
        .into_iter()
        .skip(options.offset)
        .take(options.limit.unwrap_or_default())
        .collect();

    let mut answer = json!({ field: page, "total": total });

    if notes {
        crate::memory_push::list_extras(&mut answer);
    } else {
        crate::memory_scripts_hidden::list_extras(&mut answer);
    }

    if options.places {
        answer["places"] = places;
    }

    for (name, count) in counts.as_object().into_iter().flatten() {
        let combined_only = name == "scopes" || name == "review";

        if options.combines || !combined_only {
            answer[name] = count.clone();
        }
    }
    crate::memory_push::offline(&mut answer, field);
    println!("{answer}");
    std::process::exit(0);
}

/// Strings by their text, numbers by their value.
fn compare(a: &Value, b: &Value) -> Ordering {
    match (a.as_i64(), b.as_i64()) {
        (Some(a), Some(b)) => a.cmp(&b),
        _ => a.as_str().unwrap_or_default().cmp(b.as_str().unwrap_or_default()),
    }
}

/// Whether `entry` is of note kind or script group `filter`. Script groups are disjoint: `repo` and `global` hold only trusted scripts.
fn in_group(entry: &Value, filter: &str) -> bool {
    match filter {
        "needsReview" => entry["trusted"] == false,
        "repo" | "global" => entry["trusted"] != false && entry["scope"] == filter,
        kind => entry["kind"] == kind,
    }
}

/// `needed` for a script that needs approval, else `trusted`.
fn review_of(entry: &Value) -> &'static str {
    if entry["trusted"] == false { "needed" } else { "trusted" }
}

fn scope_counts(entries: &[&Value]) -> Value {
    let count = |scope: &str| entries.iter().filter(|entry| entry["scope"] == scope).count();

    json!({ "repo": count("repo"), "global": count("global") })
}

fn kind_counts(notes: &[Value]) -> Value {
    let mut kinds: Vec<Value> = Vec::new();

    for note in notes {
        match kinds.iter_mut().find(|entry| entry["kind"] == note["kind"]) {
            Some(entry) => entry["count"] = json!(entry["count"].as_u64().unwrap_or_default() + 1),
            None => kinds.push(json!({ "kind": note["kind"], "count": 1 })),
        }
    }

    json!(kinds)
}

fn group_counts(scripts: &[Value]) -> Value {
    let count = |group: &str| scripts.iter().filter(|script| in_group(script, group)).count();

    json!({ "needsReview": count("needsReview"), "repo": count("repo"), "global": count("global") })
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

    let notes = visible_notes(options);
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

/// The notes `--hidden` asks for: the hidden team note, or the others (see [`crate::memory_push`]).
fn visible_notes(options: &Options) -> Vec<Value> {
    let notes = if options.hidden {
        crate::memory_push::hidden_fixtures()
    } else {
        note_fixtures()
            .into_iter()
            .map(|note| crate::memory_push::routed(note, false))
            .collect()
    };

    if crate::memory_push::lists("notes-useful") {
        return notes.into_iter().map(with_usefulness).collect();
    }

    notes
}
