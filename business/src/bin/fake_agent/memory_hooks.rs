//! `harness-memory hooks|routines …` stand-ins (team documents), answered from files next to the
//! executable (so each test's copy answers on its own). `<plural>` is `hooks` or `routines`:
//!
//! - Without `<plural>-config.json` every `<plural>` command is a usage error (exit 2), as a server
//!   that predates that kind of team documents answers.
//! - `<plural> config --json` prints `<plural>-config.json`.
//! - `<plural> list --json [--project P] [--offset N] [--limit N]` prints `{"total", "nextOffset",
//!   "project", "<plural>"}` from the rows in `<plural>-rows.json` (a JSON array, empty without the
//!   file): the global ones (`repo` null), plus with `--project` those of [`REPO_KEY`], the key
//!   every project gets (`project` is then `{"key": REPO_KEY, "shareable": true}`), and
//!   `nextOffset` (`null` on the last page).
//! - `<plural> get <id> --json` prints the row with that id (`notFound` without one).
//! - `<plural> put --json [--project P] [--expect-sha S]` stores the body on stdin in the rows as
//!   one of [`AUTHOR`] (of [`REPO_KEY`] with `--project`) and prints it: without `--expect-sha` an
//!   insert (`conflict` when the id exists), with it an update of the row with that hash
//!   (`conflict` for another hash, `notFound` without a row).
//! - `<plural> delete <id> [--expect-sha S] --json` removes the row and prints `{"deleted": bool}`
//!   (`conflict` for another hash).
//! - `routines delete --mine --json` removes the rows of [`AUTHOR`] and prints `{"deleted": n,
//!   "ids": [...]}`.
//! - A refusal exits 1 with `{"error", "message"}` on stdout, as the real server does. Anything
//!   else is a usage error.

use std::io::Read;
use std::path::PathBuf;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The repository key every `--project` gets.
const REPO_KEY: &str = "git:github.com/o/r";

/// The author the stand-in records for this user.
const AUTHOR: &str = "me";

/// When the stand-in stores a row.
const STORED_AT: &str = "2026-10-18T10:00:00Z";

/// `harness-memory <plural> <args>`.
pub fn docs(plural: &str, args: &[String]) -> ! {
    let Some(config) = read(&format!("{plural}-config.json")) else {
        usage();
    };

    let rows = Rows::of(plural);

    match args.first().map(String::as_str) {
        Some("config") if args.get(1..) == Some(&["--json".to_owned()][..]) => {
            println!("{config}");
            std::process::exit(0);
        }
        Some("list") => list(&rows, &args[1..]),
        Some("get") => get(&rows, &args[1..]),
        Some("put") => put(&rows, &args[1..]),
        Some("delete") => delete(&rows, &args[1..]),
        _ => usage(),
    }
}

/// The options of a command, each known one at most once.
#[derive(Default)]
struct Options {
    json: bool,
    mine: bool,
    project: bool,
    offset: usize,
    limit: Option<usize>,
    expect: Option<String>,
    positional: Vec<String>,
}

fn options(args: &[String]) -> Options {
    let mut options = Options::default();
    let mut rest = args.iter();

    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--json" => options.json = true,
            "--mine" => options.mine = true,
            "--project" => options.project = rest.next().is_some() || usage(),
            "--offset" => options.offset = number(rest.next()),
            "--limit" => options.limit = Some(number(rest.next())),
            "--expect-sha" => options.expect = Some(rest.next().cloned().unwrap_or_else(|| usage())),
            flag if flag.starts_with("--") => usage(),
            value => options.positional.push(value.to_owned()),
        }
    }

    if !options.json {
        usage();
    }

    options
}

fn list(rows: &Rows, args: &[String]) -> ! {
    let options = options(args);
    let project = options.project;

    let shown: Vec<Value> = rows
        .load()
        .into_iter()
        .filter(|row| row["repo"].is_null() || (project && row["repo"] == REPO_KEY))
        .collect();

    let limit = options.limit.unwrap_or(usize::MAX);
    let page: Vec<&Value> = shown.iter().skip(options.offset).take(limit).collect();
    let next = options.offset.saturating_add(limit);

    let mut answer = json!({
        "total": shown.len(),
        "nextOffset": (next < shown.len()).then_some(next),
        "project": project.then(|| json!({"key": REPO_KEY, "shareable": true})),
    });
    answer[rows.plural.as_str()] = json!(page);

    done(&answer)
}

fn get(rows: &Rows, args: &[String]) -> ! {
    let options = options(args);

    let [id] = options.positional.as_slice() else {
        usage();
    };

    match rows.load().into_iter().find(|row| row["id"] == id.as_str()) {
        Some(row) => done(&row),
        None => refuse("notFound", "no team document has this id"),
    }
}

fn put(rows: &Rows, args: &[String]) -> ! {
    let options = options(args);
    let mut body = String::new();

    if std::io::stdin().read_to_string(&mut body).is_err() {
        refuse("invalid", "the body is not UTF-8");
    }

    let Some(id) = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|doc| doc["id"].as_str().map(str::to_owned))
    else {
        refuse("invalid", "the body has no id");
    };

    let mut stored = rows.load();
    let position = stored.iter().position(|row| row["id"] == id.as_str());

    match (&options.expect, position) {
        (None, Some(_)) => refuse("conflict", "the team already has this id"),
        (Some(_), None) => refuse("notFound", "no team document has this id"),
        (Some(expect), Some(index)) if stored[index]["sha256"] != expect.as_str() => {
            refuse("conflict", "the team copy changed")
        }
        _ => {}
    }

    let row = json!({
        "id": id,
        "repo": options.project.then_some(REPO_KEY),
        "sha256": format!("{:x}", Sha256::digest(body.as_bytes())),
        "author": AUTHOR,
        "createdAt": STORED_AT,
        "updatedAt": STORED_AT,
        "body": body,
    });

    match position {
        Some(index) => stored[index] = row.clone(),
        None => stored.push(row.clone()),
    }

    rows.save(&stored);
    done(&row)
}

fn delete(rows: &Rows, args: &[String]) -> ! {
    let options = options(args);
    let mut stored = rows.load();

    if options.mine {
        if rows.plural != "routines" || !options.positional.is_empty() {
            usage();
        }

        let (mine, kept): (Vec<Value>, Vec<Value>) = stored.into_iter().partition(|row| row["author"] == AUTHOR);
        let ids: Vec<Value> = mine.iter().map(|row| row["id"].clone()).collect();

        rows.save(&kept);
        done(&json!({"deleted": ids.len(), "ids": ids}));
    }

    let [id] = options.positional.as_slice() else {
        usage();
    };

    let position = stored.iter().position(|row| row["id"] == id.as_str());

    if let (Some(expect), Some(index)) = (&options.expect, position)
        && stored[index]["sha256"] != expect.as_str()
    {
        refuse("conflict", "the team copy changed");
    }

    if let Some(index) = position {
        stored.remove(index);
        rows.save(&stored);
    }

    done(&json!({"deleted": position.is_some()}))
}

/// `<plural>-rows.json` next to this executable.
struct Rows {
    plural: String,
    path: Option<PathBuf>,
}

impl Rows {
    fn of(plural: &str) -> Self {
        Self {
            plural: plural.to_owned(),
            path: beside(&format!("{plural}-rows.json")),
        }
    }

    fn load(&self) -> Vec<Value> {
        self.path
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn save(&self, rows: &[Value]) {
        let Some(path) = &self.path else {
            refuse("invalid", "the stand-in has no folder");
        };

        let written = serde_json::to_string(rows)
            .map_err(|error| error.to_string())
            .and_then(|text| std::fs::write(path, text).map_err(|error| error.to_string()));

        if let Err(error) = written {
            refuse("invalid", &error);
        }
    }
}

fn number(value: Option<&String>) -> usize {
    value.and_then(|value| value.parse().ok()).unwrap_or_else(|| usage())
}

/// The file `name` next to this executable.
fn beside(name: &str) -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join(name))
}

fn read(name: &str) -> Option<String> {
    std::fs::read_to_string(beside(name)?).ok()
}

fn done(answer: &Value) -> ! {
    println!("{answer}");
    std::process::exit(0);
}

/// A refusal as the real server prints it: `{"error", "message"}` on stdout, exit 1.
fn refuse(code: &str, message: &str) -> ! {
    println!("{}", json!({"error": code, "message": message}));
    eprintln!("harness-memory: {message}");
    std::process::exit(1);
}

fn usage() -> ! {
    eprintln!("usage: harness-memory [mcp | check | notes … | scripts … | --version]");
    std::process::exit(2);
}
