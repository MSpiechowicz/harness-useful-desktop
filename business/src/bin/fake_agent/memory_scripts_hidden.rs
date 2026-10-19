//! The team scripts of the `harness-memory` stand-in, and `harness-memory scripts
//! get|hide|unhide|discard-local`, answered from fixed fixtures (nothing is kept between runs).
//!
//! - With a team database (`HARNESS_MEMORY_SCRIPTS` set, as the real server reads its routing)
//!   the scripts gain two team scripts (`place` `team`): [`TEAM_REPO_ID`] `open-prs` (the name of
//!   a local repository script) and [`TEAM_GLOBAL_ID`] `deploy`. Each script has `readOnly` (a
//!   team script while scripts are `remote+local`) and `hidden`; a list adds `hiddenCount` (1)
//!   and `localCounts` (the local fixtures by scope). Without one there are no team scripts.
//! - `scripts list --hidden` lists, and `scripts get <id> --json --hidden [--project P]` reads,
//!   the one hidden team script ([`HIDDEN_ID`]) instead; `scripts get` answers `{"script"}` with
//!   its body, and `harness-memory: script <id> not found` (exit 1) for a script it cannot see.
//! - `scripts hide|unhide <id> --json [--project P]` answers `{"hidden"|"unhidden": <id>}` for a
//!   team script it can see; a local one, an unknown one, or no team database exits 1 with the
//!   real server's line.
//! - `scripts discard-local (--all | - | IDS) [--scope S] --yes --json` answers `{"deleted": N}`
//!   while scripts are `remote` (else exit 1): every local fixture of the scope with `--all`, else
//!   the ids given (`-`: on stdin). Without `--yes` it is a usage error.

use std::io::Read;

use serde_json::{Value, json};

/// The team repository script, named as local script 12.
const TEAM_REPO_ID: i64 = 30;

/// The team global script.
const TEAM_GLOBAL_ID: i64 = 31;

/// The team script hidden on this computer.
const HIDDEN_ID: i64 = 41;

/// How scripts are routed (`remote`, `local` or `remote+local`), when a team database is set.
fn routing() -> Option<String> {
    std::env::var("HARNESS_MEMORY_SCRIPTS").ok()
}

/// One team script as the server answers it, with its body.
fn team_script(id: i64, name: &str, scope: &str, hidden: bool) -> Value {
    let repo = if scope == "repo" {
        json!("github.com/o/r")
    } else {
        Value::Null
    };
    let mut script = crate::memory::fixture(id, name, scope, repo, "make deploy\n", Some("team"));

    script["place"] = json!("team");
    script["readOnly"] = json!(routing().as_deref() == Some("remote+local"));
    script["hidden"] = json!(hidden);
    script
}

/// The visible team scripts (none without a team database).
pub fn team_scripts() -> Vec<Value> {
    if routing().is_none() {
        return Vec::new();
    }

    vec![
        team_script(TEAM_REPO_ID, "open-prs", "repo", false),
        team_script(TEAM_GLOBAL_ID, "deploy", "global", false),
    ]
}

/// The team scripts hidden on this computer (none without a team database).
pub fn hidden_scripts() -> Vec<Value> {
    if routing().is_none() {
        return Vec::new();
    }

    vec![team_script(HIDDEN_ID, "hidden-deploy", "global", true)]
}

/// The scripts of the local file: the fixtures saved on this computer.
fn local_scripts() -> Vec<Value> {
    crate::memory::fixtures()
        .into_iter()
        .filter(|script| script["place"] == "local")
        .collect()
}

/// The fields a scripts list adds while a team database is set.
pub fn list_extras(answer: &mut Value) {
    if routing().is_none() {
        return;
    }

    let local = local_scripts();
    let count = |scope: &str| local.iter().filter(|script| script["scope"] == scope).count();

    answer["hiddenCount"] = json!(hidden_scripts().len());
    answer["localCounts"] = json!({ "repo": count("repo"), "global": count("global") });
}

/// How many of `entries` are read-only team scripts.
pub fn read_only_count(entries: &[&Value]) -> usize {
    entries.iter().filter(|entry| entry["readOnly"] == true).count()
}

/// `scripts <command> <args>` for `get`, `hide`, `unhide` and `discard-local`.
pub fn command(command: &str, args: &[String]) -> ! {
    let call = Call::read(args);

    match command {
        "get" => get(&call),
        "hide" | "unhide" => hide(command, &call),
        "discard-local" => discard_local(&call),
        _ => usage(),
    }
}

/// The arguments of one command.
#[derive(Default)]
struct Call {
    /// The id, ids, `-` or `--all`.
    target: Option<String>,
    switches: Vec<String>,
    scope: Option<String>,
    project: bool,
}

impl Call {
    fn read(args: &[String]) -> Self {
        let mut call = Self::default();
        let mut rest = args.iter();

        while let Some(arg) = rest.next() {
            match arg.as_str() {
                "--project" => call.project = rest.next().is_some_and(|path| !path.is_empty()),
                "--scope" => call.scope = Some(rest.next().cloned().unwrap_or_else(|| usage())),
                "--json" | "--yes" | "--hidden" => call.switches.push(arg.clone()),
                target if call.target.is_none() && (target == "--all" || !target.starts_with("--")) => {
                    call.target = Some(target.to_owned());
                }
                _ => usage(),
            }
        }

        call
    }

    fn has(&self, switch: &str) -> bool {
        self.switches.iter().any(|given| given == switch)
    }

    fn id(&self) -> i64 {
        self.target
            .as_deref()
            .and_then(|id| id.parse().ok())
            .unwrap_or_else(|| usage())
    }

    /// The script `id` names among `scripts`, as the arguments' project sees them.
    fn find(&self, scripts: Vec<Value>) -> Option<Value> {
        let id = self.id();

        scripts
            .into_iter()
            .find(|script| script["id"] == id && (self.project || script["scope"] != "repo"))
    }
}

/// `scripts get <id> --json [--project P] [--hidden]`.
fn get(call: &Call) -> ! {
    if !call.has("--json") {
        usage();
    }

    let scripts = if call.has("--hidden") {
        hidden_scripts()
    } else {
        crate::memory::fixtures()
    };

    match call.find(scripts) {
        Some(script) => answer(json!({ "script": script })),
        None => fail(&format!("harness-memory: script {} not found", call.id())),
    }
}

/// `scripts hide|unhide <id> --json [--project P]`.
fn hide(command: &str, call: &Call) -> ! {
    if !call.has("--json") {
        usage();
    }

    if routing().is_none() {
        fail("harness-memory: no team database is set");
    }

    let id = call.id();
    let mut scripts = crate::memory::fixtures();
    scripts.extend(hidden_scripts());

    match call.find(scripts) {
        None => fail(&format!("harness-memory: script {id} not found")),
        Some(script) if script["place"] != "team" => fail(&format!(
            "harness-memory: script {id} is saved on this computer, delete it instead"
        )),
        Some(_) if command == "hide" => answer(json!({ "hidden": id })),
        Some(_) => answer(json!({ "unhidden": id })),
    }
}

/// `scripts discard-local (--all | - | IDS) [--scope S] --yes --json`.
fn discard_local(call: &Call) -> ! {
    if !call.has("--yes") || !call.has("--json") {
        usage();
    }

    if routing().as_deref() != Some("remote") {
        fail("harness-memory: scripts are not set to \"remote\", their local copies are still read");
    }

    let in_scope = |script: &Value| call.scope.as_deref().is_none_or(|scope| script["scope"] == scope);

    let deleted = match call.target.as_deref() {
        Some("--all") => local_scripts().iter().filter(|script| in_scope(script)).count(),
        Some("-") => {
            let mut stdin = String::new();
            std::io::stdin().read_to_string(&mut stdin).unwrap_or_else(|_| usage());
            stdin.trim().split(',').count()
        }
        Some(ids) => ids.split(',').count(),
        None => usage(),
    };

    answer(json!({ "deleted": deleted }))
}

fn answer(data: Value) -> ! {
    println!("{data}");
    std::process::exit(0);
}

fn fail(line: &str) -> ! {
    eprintln!("{line}");
    std::process::exit(1);
}

fn usage() -> ! {
    eprintln!("usage: harness-memory scripts [get | hide | unhide | discard-local] …");
    std::process::exit(2);
}
