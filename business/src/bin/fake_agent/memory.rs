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
//!   server answers).
//! - `script_list` needs `scope: "all"`; it answers the global scripts, plus the repository one
//!   when `project_path` is set, plus one entry of an unknown scope.
//! - `script_get` and `script_delete` take `id`; an unknown id is an `isError` result with the
//!   real server's text (`script <id> not found`), and id [`FAILING_ID`] is an `isError` result
//!   of another kind whose text holds a URL with a password.
//! - `FAKE_MEMORY_VERSION` replaces the `0.0.0` of `--version` and `serverInfo`.
//! - `harness-memory trust <id>=<sha256>… [--project <path>]` ([`trust`]) prints `trusted <id>`
//!   for each fixture id given with its `sha256` and exits 0; a fixture id with another hash exits 1
//!   with `harness-memory: script <id> changed since you reviewed it`, an unknown id exits 1 with
//!   `error: script <id> not found`, id [`FAILING_ID`] exits 1 with a URL with a password on
//!   stderr, and an argument without `=` exits 2.

use std::io::{BufRead, Write};

use serde_json::{Map, Value, json};

const NOTE_TOOLS: [&str; 3] = ["note_save", "note_search", "note_delete"];
const SCRIPT_TOOLS: [&str; 5] = [
    "script_save",
    "script_list",
    "script_get",
    "script_delete",
    "script_run",
];

/// The id whose `script_get` and `script_delete` fail as a broken team database would.
const FAILING_ID: i64 = 500;

/// The version the stand-in reports.
pub fn version() -> String {
    std::env::var("FAKE_MEMORY_VERSION").unwrap_or_else(|_| "0.0.0".to_owned())
}

/// Answers MCP requests on stdin until it closes, then exits 0.
pub fn serve() -> ! {
    let legacy = std::env::var("FAKE_MEMORY_TOOLS").is_ok_and(|tools| tools == "legacy");
    let mut scripts = fixtures();

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
            "tools/call" => call(&mut scripts, legacy, params),
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

fn call(scripts: &mut Vec<Value>, legacy: bool, params: &Value) -> Result<Value, (i64, String)> {
    let name = params["name"].as_str().unwrap_or_default();
    let arguments = &params["arguments"];

    if legacy || !SCRIPT_TOOLS.contains(&name) {
        return Err((-32602, format!("Unknown tool: {name}")));
    }

    if arguments["id"] == FAILING_ID {
        return Ok(tool_error(
            "the team database cannot be reached (postgres://team:hunter2@db.example.com/memory)".to_owned(),
        ));
    }

    let position = arguments["id"]
        .as_i64()
        .and_then(|id| scripts.iter().position(|script| script["id"] == id));

    match (name, position) {
        ("script_list", _) => Ok(list(scripts, arguments)),
        ("script_get", Some(index)) => Ok(structured(json!({ "script": scripts[index] }))),
        ("script_delete", Some(index)) => {
            let removed = scripts.remove(index);
            Ok(structured(json!({ "deleted": true, "id": removed["id"] })))
        }
        _ => Ok(tool_error(format!("script {} not found", arguments["id"]))),
    }
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

/// The stand-in's scripts, with their bodies.
pub fn fixtures() -> Vec<Value> {
    let script = |id: i64, name: &str, scope: &str, repo: Value, body: &str, approval: Option<&str>| {
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
    };

    vec![
        script(
            12,
            "open-prs",
            "repo",
            json!("github.com/o/r"),
            "gh pr list --state open\n",
            Some("local"),
        ),
        script(
            7,
            "release-notes",
            "global",
            Value::Null,
            "git log --oneline -20\n",
            None,
        ),
        script(99, "odd-scope", "workspace", Value::Null, "true\n", Some("team")),
    ]
}
