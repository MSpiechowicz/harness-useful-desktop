//! `harness-memory hooks config|list` stand-ins, answered from files next to the executable (so
//! each test's copy answers on its own):
//!
//! - Without `hooks-config.json` every `hooks` command is a usage error (exit 2), as a server that
//!   predates team hooks answers.
//! - `hooks config --json` prints `hooks-config.json`.
//! - `hooks list --json [--project P] [--offset N] [--limit N]` prints `{"total", "nextOffset",
//!   "project", "hooks"}` from the rows in `hooks-rows.json` (a JSON array, empty without the file): the
//!   global ones (`repo` null), plus with `--project` those of [`REPO_KEY`], the key every project
//!   gets (`project` is then `{"key": REPO_KEY, "shareable": true}`), and `nextOffset` (`null` on
//!   the last page).
//! - Anything else is a usage error.

use std::path::PathBuf;

use serde_json::{Value, json};

/// The repository key every `--project` gets.
const REPO_KEY: &str = "git:github.com/o/r";

/// `harness-memory hooks <args>`.
pub fn hooks(args: &[String]) -> ! {
    let Some(config) = read("hooks-config.json") else {
        usage();
    };

    match args.first().map(String::as_str) {
        Some("config") if args.get(1..) == Some(&["--json".to_owned()][..]) => {
            println!("{config}");
            std::process::exit(0);
        }
        Some("list") => list(&args[1..]),
        _ => usage(),
    }
}

fn list(args: &[String]) -> ! {
    let mut project = false;
    let mut offset = 0;
    let mut limit = usize::MAX;
    let mut json = false;
    let mut rest = args.iter();

    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--project" => project = rest.next().is_some() || usage(),
            "--offset" => offset = number(rest.next()),
            "--limit" => limit = number(rest.next()),
            _ => usage(),
        }
    }

    if !json {
        usage();
    }

    let rows: Vec<Value> = read("hooks-rows.json")
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();

    let shown: Vec<Value> = rows
        .into_iter()
        .filter(|row| row["repo"].is_null() || (project && row["repo"] == REPO_KEY))
        .collect();

    let page: Vec<&Value> = shown.iter().skip(offset).take(limit).collect();
    let next = offset.saturating_add(limit);
    let answer = json!({
        "total": shown.len(),
        "nextOffset": (next < shown.len()).then_some(next),
        "project": project.then(|| json!({"key": REPO_KEY, "shareable": true})),
        "hooks": page,
    });

    println!("{answer}");
    std::process::exit(0);
}

fn number(value: Option<&String>) -> usize {
    value.and_then(|value| value.parse().ok()).unwrap_or_else(|| usage())
}

/// The file `name` next to this executable.
fn read(name: &str) -> Option<String> {
    let dir: PathBuf = std::env::current_exe().ok()?.parent()?.to_path_buf();

    std::fs::read_to_string(dir.join(name)).ok()
}

fn usage() -> ! {
    eprintln!("usage: harness-memory [mcp | check | notes … | scripts … | --version]");
    std::process::exit(2);
}
