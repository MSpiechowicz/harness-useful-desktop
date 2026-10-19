//! `harness-memory mcp` stand-in (also `fake_agent --dialect memory`): a JSON-RPC MCP server
//! with a fixed set of scripts kept in memory for the life of the process.
//!
//! - `tools/list` offers the note tools and `script_save`, `script_list`, `script_get`,
//!   `script_delete`, `script_run` (whose arguments have no `sha`: the server asks the user before
//!   running a script); with `FAKE_MEMORY_TOOLS=agent-approval` `script_run` takes `sha` (a server
//!   that let the agent approve scripts); with `FAKE_MEMORY_TOOLS=no-run` there is no `script_run`,
//!   with `FAKE_MEMORY_TOOLS=bare-run` its `inputSchema` lists no properties (servers whose
//!   approvals cannot be told); with `FAKE_MEMORY_TOOLS=legacy` only the note tools (a server that
//!   predates scripts), and calling a script tool is a JSON-RPC error.
//! - Script 12 is `approval: "local"`; script 7 is not trusted and has no `approval` (as an older
//!   server answers). With a team database (`HARNESS_MEMORY_SCRIPTS` set) team scripts follow, see
//!   [`crate::memory_scripts_hidden`]; `script_save` never replaces a read-only one.
//! - `script_list` needs `scope: "all"`; it answers the global scripts, plus the repository one
//!   when `project_path` is set, plus one entry of an unknown scope.
//! - `script_get` and `script_delete` take `id`; an unknown id is an `isError` result with the
//!   real server's text (`script <id> not found`), and id [`FAILING_ID`] is an `isError` result
//!   of another kind whose text holds a URL with a password. `script_get` and `script_run` also
//!   take `name` (an unknown one gets the real server's `no script named …` text). `script_run`
//!   of an untrusted script is refused like the real server's (`untrusted: true`), a trusted one
//!   "runs": stdout `ran <first word of its body>`, exit code 0.
//! - `script_save` replaces the script of that name in that scope or adds one (see [`save`]); a
//!   `name` with `scope: "repo"` or `"global"` finds a script of that scope only. Replacing an
//!   untrusted script with the same body (byte for byte) keeps it `approval: "needed"`, with the
//!   real server's trust hash; `FAKE_MEMORY_SEEN_UNAPPROVED` does so for any body (the server's
//!   "its code waited for approval on this machine before").
//!   `FAKE_MEMORY_SAVED_REPO` stands for a server that stored the script elsewhere: the saved
//!   script has that repository.
//! - `FAKE_MEMORY_TRUSTED_SHA` stands for the approvals file holding that hash: `script_get` of a
//!   script with it answers `approval: "approved"`, and `trust` takes any id given with it.
//! - `note_save` and `note_update` answer as the real server does, see [`crate::memory_notes`].
//! - `FAKE_MEMORY_VERSION` replaces the `0.0.0` of `--version` and `serverInfo`.
//! - `harness-memory trust <id>=<sha256>… [--project <path>]` ([`trust`]) prints `trusted <id>`
//!   for each fixture id given with its `sha256` and exits 0; a fixture id with another hash exits 1
//!   with `harness-memory: script <id> changed since you reviewed it`, an unknown id exits 1 with
//!   `error: script <id> not found`, id [`FAILING_ID`] exits 1 with a URL with a password on
//!   stderr, and an argument without `=` exits 2.

use std::io::{BufRead, Write};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const NOTE_TOOLS: [&str; 4] = ["note_save", "note_update", "note_search", "note_delete"];
const SCRIPT_TOOLS: [&str; 5] = [
    "script_save",
    "script_list",
    "script_get",
    "script_delete",
    "script_run",
];

/// The id whose `script_get` and `script_delete` fail as a broken team database would.
const FAILING_ID: i64 = 500;

/// The hash `FAKE_MEMORY_TRUSTED_SHA` approves, if set.
fn trusted_sha() -> Option<String> {
    std::env::var("FAKE_MEMORY_TRUSTED_SHA").ok()
}

/// The real server's trust hash of a version: SHA-256 of `harness-memory-script-v1 NUL
/// <language> NUL <body>`.
fn trust_sha256(language: &str, body: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"harness-memory-script-v1\0");
    hash.update(language.as_bytes());
    hash.update([0]);
    hash.update(body.as_bytes());

    format!("{:x}", hash.finalize())
}

/// The version the stand-in reports.
pub fn version() -> String {
    std::env::var("FAKE_MEMORY_VERSION").unwrap_or_else(|_| "0.0.0".to_owned())
}

/// Answers MCP requests on stdin until it closes, then exits 0.
pub fn serve() -> ! {
    let legacy = std::env::var("FAKE_MEMORY_TOOLS").is_ok_and(|tools| tools == "legacy");
    let mut scripts = fixtures();
    let mut notes = crate::memory_cli::note_fixtures();

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };

        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        // Notifications (`notifications/initialized`) have no id and get no answer.
        let Some(id) = message.get("id").cloned() else {
            continue;
        };

        let method = message["method"].as_str().unwrap_or_default();
        let params = &message["params"];

        let answer = match method {
            "initialize" => Ok(json!({
                "protocolVersion": "2025-11-25",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "harness-memory", "version": version() },
            })),
            "tools/list" => Ok(tools(legacy)),
            "tools/call" => call(&mut scripts, &mut notes, legacy, params),
            _ => Err((-32601, "Method not found".to_owned())),
        };

        let frame = match answer {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => {
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
            }
        };

        let mut stdout = std::io::stdout().lock();
        let written = writeln!(stdout, "{frame}").and_then(|()| stdout.flush());

        if written.is_err() {
            break;
        }
    }

    std::process::exit(0);
}

fn tools(legacy: bool) -> Value {
    let mode = std::env::var("FAKE_MEMORY_TOOLS").unwrap_or_default();
    let script_tools: &[&str] = if legacy { &[] } else { &SCRIPT_TOOLS };

    let tools: Vec<Value> = NOTE_TOOLS
        .iter()
        .chain(script_tools)
        .filter(|name| !(**name == "script_run" && mode == "no-run"))
        .map(|name| {
            if *name == "script_run" && mode == "bare-run" {
                return json!({ "name": name, "inputSchema": { "type": "object" } });
            }

            let mut properties = json!({ "id": { "type": "integer" }, "project_path": { "type": "string" } });

            if *name == "script_run" && mode == "agent-approval" {
                properties["sha"] = json!({ "type": "string" });
            }

            json!({ "name": name, "inputSchema": { "type": "object", "properties": properties } })
        })
        .collect();

    json!({ "tools": tools })
}

/// `harness-memory trust <id>=<sha256>… [--project <path>]` (see the module docs).
pub fn trust(args: &[String]) -> ! {
    let scripts = fixtures();
    let approved = trusted_sha();
    let sha_of = |id: i64| {
        scripts
            .iter()
            .find(|script| script["id"].as_i64() == Some(id))
            .and_then(|script| script["sha256"].as_str())
    };
    let mut ids = Vec::new();
    let mut rest = args.iter();

    while let Some(arg) = rest.next() {
        if arg == "--project" {
            if rest.next().is_none() {
                exit_with("error: --project needs a path", 2);
            }
            continue;
        }

        let Some((id, sha)) = arg.split_once('=') else {
            exit_with("error: give each script as <id>=<sha256>", 2);
        };

        let parsed = id.parse::<i64>().ok();

        match (parsed, parsed.and_then(sha_of)) {
            (Some(FAILING_ID), _) => exit_with(
                "error: the team database cannot be reached (postgres://team:hunter2@db.example.com/memory)",
                1,
            ),
            (Some(id), _) if approved.as_deref() == Some(sha) => ids.push(id),
            (Some(id), Some(current)) if current == sha => ids.push(id),
            (Some(id), Some(_)) => exit_with(&format!("harness-memory: script {id} changed since you reviewed it"), 1),
            _ => exit_with(&format!("error: script {id} not found"), 1),
        }
    }

    if ids.is_empty() {
        exit_with("error: name the scripts to trust", 2);
    }

    for id in ids {
        println!("trusted {id}");
    }

    std::process::exit(0);
}

fn exit_with(message: &str, code: i32) -> ! {
    eprintln!("{message}");
    std::process::exit(code);
}

fn call(
    scripts: &mut Vec<Value>,
    notes: &mut Vec<Value>,
    legacy: bool,
    params: &Value,
) -> Result<Value, (i64, String)> {
    let name = params["name"].as_str().unwrap_or_default();
    let arguments = &params["arguments"];

    if let Some(answer) = crate::memory_notes::call(notes, name, arguments) {
        return Ok(answer);
    }

    if legacy || !SCRIPT_TOOLS.contains(&name) {
        return Err((-32602, format!("Unknown tool: {name}")));
    }

    if arguments["id"] == FAILING_ID {
        return Ok(tool_error(
            "the team database cannot be reached (postgres://team:hunter2@db.example.com/memory)".to_owned(),
        ));
    }

    if name == "script_save" {
        return Ok(save(scripts, arguments));
    }

    // A scope of `repo` or `global` keeps that scope only, as the real server's view does.
    let in_scope = |script: &Value| match arguments["scope"].as_str() {
        Some(scope @ ("repo" | "global")) => script["scope"] == scope,
        _ => true,
    };

    let position = match arguments["name"].as_str() {
        Some(wanted) => scripts
            .iter()
            .position(|script| script["name"] == wanted && in_scope(script)),
        None => arguments["id"]
            .as_i64()
            .and_then(|id| scripts.iter().position(|script| script["id"] == id)),
    };

    match (name, position) {
        ("script_list", _) => Ok(list(scripts, arguments)),
        ("script_get", Some(index)) => Ok(structured(json!({ "script": with_approval(&scripts[index]) }))),
        ("script_run", Some(index)) => Ok(run(&scripts[index])),
        ("script_delete", Some(index)) => {
            let removed = scripts.remove(index);
            Ok(structured(json!({ "deleted": true, "id": removed["id"] })))
        }
        (_, None) if arguments["name"].is_string() => Ok(tool_error(format!(
            "no script named {} here or in the global scope",
            arguments["name"].as_str().unwrap_or_default()
        ))),
        _ => Ok(tool_error(format!("script {} not found", arguments["id"]))),
    }
}

/// `script_run` of `script`: refused as the real server refuses an untrusted script when no
/// Harness Desktop answers, else a run that printed its body's first word and exited 0.
fn run(script: &Value) -> Value {
    if script["trusted"] != true {
        let text = format!("script {} is not approved on this machine", script["name"]);
        let refusal = json!({ "untrusted": true, "id": script["id"], "name": script["name"], "pending": false });

        return json!({ "content": [{ "type": "text", "text": text }], "isError": true, "structuredContent": refusal });
    }

    let body = script["body"].as_str().unwrap_or_default();
    let first = body.split_whitespace().next().unwrap_or_default();

    structured(json!({
        "id": script["id"],
        "sha256": script["sha256"],
        "exitCode": 0,
        "timedOut": false,
        "durationMs": 5,
        "stdout": format!("ran {first}\n"),
        "stderr": "",
        "stdoutTruncated": false,
        "stderrTruncated": false,
    }))
}

/// `script`, approved when its hash is `FAKE_MEMORY_TRUSTED_SHA`.
fn with_approval(script: &Value) -> Value {
    let mut script = script.clone();

    if trusted_sha().is_some_and(|sha| script["sha256"] == sha.as_str()) {
        script["trusted"] = json!(true);
        script["approval"] = json!("approved");
    }

    script
}

/// `script_save`: replaces the script of that name in that scope, or adds one, saved here (so
/// trusted, `approval: "local"`); answers `{script, created}` as the real server does. A body
/// `fail` is refused as a body the server will not keep. The code of an untrusted script it
/// replaces stays untrusted (`approval: "needed"`, the real trust hash), as the real server's does.
fn save(scripts: &mut Vec<Value>, arguments: &Value) -> Value {
    let scope = arguments["scope"].as_str().unwrap_or("repo");
    let name = arguments["name"].as_str().unwrap_or_default();

    if arguments["body"] == "fail" {
        return tool_error("the script body has a hidden character".to_owned());
    }

    let position = scripts
        .iter()
        .position(|script| script["name"] == name && script["scope"] == scope && script["readOnly"] != true);
    let id = position.map_or_else(
        || {
            scripts
                .iter()
                .filter_map(|script| script["id"].as_i64())
                .max()
                .unwrap_or(0)
                + 1
        },
        |index| scripts[index]["id"].as_i64().unwrap_or_default(),
    );

    let repo = if scope == "repo" {
        json!("github.com/o/r")
    } else {
        Value::Null
    };
    let body = arguments["body"].as_str().unwrap_or_default();
    let mut script = fixture(id, name, scope, repo, body, Some("local"));
    script["description"] = arguments["description"].clone();
    script["language"] = arguments["language"].clone();
    script["timeoutSecs"] = arguments["timeout_secs"].clone();
    script["runCount"] = json!(0);

    // Like the server: only a byte-for-byte copy of code waiting for approval stays untrusted, or
    // any code when `FAKE_MEMORY_SEEN_UNAPPROVED` says its code waited for approval here before.
    let seen = std::env::var_os("FAKE_MEMORY_SEEN_UNAPPROVED").is_some();

    if position.is_some_and(|index| scripts[index]["trusted"] == false && (seen || scripts[index]["body"] == body)) {
        let language = arguments["language"].as_str().unwrap_or_default();
        script["sha256"] = json!(trust_sha256(language, body));
        script["trusted"] = json!(false);
        script["approval"] = json!("needed");
    }

    if let Ok(repo) = std::env::var("FAKE_MEMORY_SAVED_REPO") {
        script["repo"] = json!(repo);
    }

    match position {
        Some(index) => scripts[index] = script.clone(),
        None => scripts.push(script.clone()),
    }

    let mut summary = script;
    summary.as_object_mut().map(|fields| fields.remove("body"));
    let text = format!("saved script {id} {name}");

    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": { "script": summary, "created": position.is_none() },
    })
}

fn tool_error(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": true })
}

fn list(scripts: &[Value], arguments: &Value) -> Value {
    if arguments["scope"] != "all" {
        return json!({ "content": [{ "type": "text", "text": "expected scope all" }], "isError": true });
    }

    let with_repo = arguments["project_path"].is_string();
    let listed: Vec<Value> = scripts
        .iter()
        .filter(|script| with_repo || script["scope"] != "repo")
        .map(|script| {
            let mut summary = script.clone();
            summary.as_object_mut().map(|fields| fields.remove("body"));
            summary
        })
        .collect();

    structured(json!({ "scripts": listed }))
}

fn structured(data: Value) -> Value {
    json!({ "content": [{ "type": "text", "text": "agent-written data" }], "structuredContent": data })
}

/// The stand-in's scripts, with their bodies: those of this computer, then the team's.
pub fn fixtures() -> Vec<Value> {
    let mut scripts = vec![
        fixture(
            12,
            "open-prs",
            "repo",
            json!("github.com/o/r"),
            "gh pr list --state open\n",
            Some("local"),
        ),
        fixture(
            7,
            "release-notes",
            "global",
            Value::Null,
            "git log --oneline -20\n",
            None,
        ),
        fixture(99, "odd-scope", "workspace", Value::Null, "true\n", Some("team")),
    ];

    scripts.extend(crate::memory_scripts_hidden::team_scripts());
    scripts
}

/// One script as the server answers it, with its body.
pub fn fixture(id: i64, name: &str, scope: &str, repo: Value, body: &str, approval: Option<&str>) -> Value {
    let mut fields = Map::new();
    fields.insert("id".to_owned(), json!(id));
    fields.insert("name".to_owned(), json!(name));
    fields.insert("description".to_owned(), json!(format!("{name} fixture")));
    fields.insert("language".to_owned(), json!("bash"));
    fields.insert("scope".to_owned(), json!(scope));
    fields.insert("repo".to_owned(), repo);
    fields.insert("sha256".to_owned(), json!(format!("{id:064x}")));
    fields.insert("timeoutSecs".to_owned(), json!(60));
    fields.insert("place".to_owned(), json!("local"));
    fields.insert("author".to_owned(), json!({ "source": "claude-code", "root": null }));
    fields.insert("createdAt".to_owned(), json!("2026-10-01T08:00:00Z"));
    fields.insert("updatedAt".to_owned(), json!("2026-10-02T09:30:00Z"));
    fields.insert("runCount".to_owned(), json!(id % 5));
    fields.insert("lastRunAt".to_owned(), Value::Null);
    fields.insert("lastExitCode".to_owned(), Value::Null);
    fields.insert("trusted".to_owned(), json!(approval.is_some()));
    if let Some(approval) = approval {
        fields.insert("approval".to_owned(), json!(approval));
    }
    fields.insert("body".to_owned(), json!(body));
    Value::Object(fields)
}
