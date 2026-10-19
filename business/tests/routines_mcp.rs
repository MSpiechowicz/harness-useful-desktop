//! The routine tools of the MCP server mode (`harness-desktop mcp`): the tool list and its
//! schemas, and agents adding, listing, changing and removing routines under the agent rules of
//! `docs/routines-format.md` (pending until approved, `createdBy` kept, the sha256 precondition,
//! removal only of routines an agent created, a user's routine kept where it applies, secrets
//! masked, team routines never shown), with `project` turned into a project root.

mod routine_support;

use std::path::PathBuf;
use std::time::SystemTime;

use harness_business::hooks_mcp::Server;
use harness_business::model::{HookAuthor, HookScope, RoutineDraft};
use harness_business::routines::APPROVALS_FILE;
use pretty_assertions::assert_eq;
use routine_support::{Root, agent_action, body, command_draft, command_json, file, rid, root, team_row};
use serde_json::{Value, json};

/// A routines root, and the server started in a subfolder of its project.
struct Machine {
    root: Root,
    server: Server,
    deep: PathBuf,
}

fn machine() -> Machine {
    let root = root();
    let deep = PathBuf::from(&root.project).join("src/deep");
    std::fs::create_dir_all(&deep).unwrap();

    let server = Server::new(None, Some(deep.clone())).with_routines(Some(root.store.clone()));

    Machine { root, server, deep }
}

impl Machine {
    /// The result of one `tools/call`.
    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        rpc(
            &mut self.server,
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        )
    }

    /// The reply object of a call that succeeded.
    fn ok(&mut self, tool: &str, arguments: Value) -> Value {
        let result = self.call(tool, arguments);
        assert_eq!(result["isError"], json!(false), "{}", shown(&result));
        result["structuredContent"].clone()
    }

    /// The refusal text of a call that failed.
    fn refused(&mut self, tool: &str, arguments: Value) -> String {
        let result = self.call(tool, arguments);
        assert_eq!(result["isError"], json!(true), "{tool} {}", shown(&result));
        shown(&result).to_owned()
    }

    /// A routine the user created in Harness Desktop (approved): `(id, sha256)`.
    fn desktop_routine(&self, draft: &RoutineDraft) -> (String, String) {
        let view = self
            .root
            .store
            .save(draft, HookAuthor::Desktop, SystemTime::now())
            .unwrap();
        (view.id, view.sha256)
    }
}

/// The result of one request.
fn rpc(server: &mut Server, method: &str, params: Value) -> Value {
    let request = json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params });
    server.handle(request).expect("a request is answered")["result"].clone()
}

fn shown(result: &Value) -> &str {
    result["content"][0]["text"].as_str().unwrap()
}

/// A morning agent routine of the working folder's project.
fn add_args() -> Value {
    json!({
        "name": "Morning review",
        "scope": "project",
        "when": "weekdays",
        "at": "09:00",
        "action": "agent",
        "harness": "omp",
        "prompt": "List what changed since yesterday.",
    })
}

/// `base` with the fields of `change` set (a `null` removes one).
fn with(base: Value, change: Value) -> Value {
    let mut base = base;
    for (key, value) in change.as_object().unwrap() {
        match value {
            Value::Null => base.as_object_mut().unwrap().remove(key),
            value => base.as_object_mut().unwrap().insert(key.clone(), value.clone()),
        };
    }
    base
}

#[test]
fn tools_list_has_the_routine_tools_with_flat_arguments() {
    let mut machine = machine();
    let listed = rpc(&mut machine.server, "tools/list", json!({}));
    let tools = listed["tools"].as_array().unwrap();

    let routine = |name: &str| tools.iter().find(|tool| tool["name"] == name).unwrap().clone();
    let required = |name: &str| routine(name)["inputSchema"]["required"].clone();

    assert_eq!(required("routine_list"), json!([]));
    assert_eq!(required("routine_get"), json!(["id"]));
    assert_eq!(required("routine_add"), json!(["name", "scope", "when", "action"]));
    assert_eq!(required("routine_update"), json!(["id", "sha256"]));
    assert_eq!(required("routine_remove"), json!(["id", "sha256"]));

    let add = routine("routine_add");
    let properties = add["inputSchema"]["properties"].as_object().unwrap();
    let names: Vec<&str> = properties.keys().map(String::as_str).collect();
    assert_eq!(
        names.join(" "),
        "name description scope project when at every cron action prompt harness model approvalMode trustOnce \
         maxMinutes command timeoutSecs script scriptScope enabled"
    );
    assert_eq!(
        add["inputSchema"]["properties"]["when"]["enum"],
        json!(["daily", "weekdays", "everyMinutes", "everyHours", "cron", "appStart"])
    );
    assert_eq!(
        add["inputSchema"]["properties"]["harness"]["enum"],
        json!(["omp", "pi", "opencode"])
    );
    assert_eq!(add["inputSchema"]["additionalProperties"], json!(false));

    let update = routine("routine_update");
    assert!(update["inputSchema"]["properties"]["sha256"].is_object());
    assert!(
        update["inputSchema"]["properties"]["enabled"].get("default").is_none(),
        "an edit has no defaults"
    );

    for (name, read_only) in [("routine_list", true), ("routine_get", true), ("routine_add", false)] {
        assert_eq!(routine(name)["annotations"]["readOnlyHint"], json!(read_only), "{name}");
    }
    assert_eq!(routine("routine_remove")["annotations"]["destructiveHint"], json!(true));
    assert!(
        tools.iter().all(|tool| !tool["name"].as_str().unwrap().contains("run")),
        "no tool runs a routine"
    );

    let initialized = rpc(&mut machine.server, "initialize", json!({}));
    let instructions = initialized["instructions"].as_str().unwrap();
    assert!(instructions.contains("Routines view"), "{instructions}");
    assert!(instructions.contains("Ask first"), "{instructions}");
    assert!(!instructions.contains(';'), "no sentences joined by a semicolon");
}

#[test]
fn an_added_routine_is_pending_in_the_project_of_the_working_folder() {
    let mut machine = machine();

    let added = machine.ok("routine_add", add_args())["added"].clone();
    assert_eq!(added["status"], "pending");
    assert_eq!(added["approval"], "pending");
    assert_eq!(added["createdBy"], "agent");
    assert_eq!(added["project"], json!(machine.root.project), "the repository root");
    assert_eq!(added["when"], json!({ "type": "weekdays", "at": "09:00" }));
    assert_eq!(added["action"]["harness"], "omp");
    assert_eq!(added["action"]["model"], Value::Null);
    assert_eq!(added["enabled"], json!(true));

    let listed = machine.ok("routine_list", json!({}));
    assert_eq!(listed["project"], json!(machine.root.project));
    assert_eq!(listed["projectRoutines"], "1 of 64");
    assert_eq!(listed["global"], "0 of 64");
    assert_eq!(listed["routines"][0]["id"], added["id"]);
    assert!(listed["note"].as_str().unwrap().contains("Routines view"));

    let global_only = machine.ok("routine_list", json!({ "scope": "global" }));
    assert_eq!(global_only["routines"], json!([]));

    let got = machine.ok("routine_get", json!({ "id": added["id"] }));
    assert_eq!(got["routine"]["sha256"], added["sha256"]);

    let approvals = machine.root.store.config_dir().join(APPROVALS_FILE);
    assert!(!approvals.exists(), "the server never writes the approvals");
}

#[test]
fn project_is_turned_into_its_project_root() {
    let mut machine = machine();
    let deep = machine.deep.to_string_lossy().into_owned();

    let added = machine.ok("routine_add", with(add_args(), json!({ "project": deep })))["added"].clone();
    assert_eq!(added["project"], json!(machine.root.project));

    let other = machine.root.temp.path().join("other");
    std::fs::create_dir_all(&other).unwrap();
    let other = dunce::canonicalize(other).unwrap().to_string_lossy().into_owned();
    let elsewhere = machine.ok("routine_add", with(add_args(), json!({ "project": other })))["added"].clone();
    assert_eq!(elsewhere["project"], json!(other));

    let here = machine.ok("routine_list", json!({}));
    assert_eq!(
        here["routines"].as_array().unwrap().len(),
        1,
        "another project is not listed"
    );
    let same = machine.ok("routine_list", json!({ "project": deep }));
    assert_eq!(same["routines"], here["routines"], "a folder of this project");

    let relative = machine.refused("routine_add", with(add_args(), json!({ "project": "src" })));
    assert!(relative.contains("absolute"), "{relative}");

    let nowhere = machine.root.temp.path().join("nowhere").to_string_lossy().into_owned();
    let missing = machine.refused("routine_add", with(add_args(), json!({ "project": nowhere })));
    assert!(missing.contains("does not exist"), "{missing}");

    let global = with(add_args(), json!({ "scope": "global", "project": deep }));
    let refused = machine.refused("routine_add", global);
    assert!(refused.contains("scope project"), "{refused}");
}

#[test]
fn an_agent_edit_of_a_user_routine_keeps_where_it_applies_and_makes_it_pending() {
    let mut machine = machine();
    let mut draft = command_draft("Nightly report");
    draft.scope = HookScope::Project;
    draft.project = Some(machine.root.project.clone());
    let (id, sha256) = machine.desktop_routine(&draft);

    let got = machine.ok("routine_get", json!({ "id": id }))["routine"].clone();
    assert_eq!(got["status"], "active");
    assert_eq!(got["approval"], "approved");

    let updated = machine.ok(
        "routine_update",
        json!({ "id": id, "sha256": sha256, "command": "make report-all" }),
    )["updated"]
        .clone();
    assert_eq!(updated["status"], "pending");
    assert_eq!(updated["createdBy"], "desktop");
    assert_eq!(updated["updatedBy"], "agent");
    assert_eq!(updated["scope"], "project");
    assert_eq!(updated["project"], json!(machine.root.project));
    assert_eq!(
        updated["when"],
        json!({ "type": "daily", "at": "07:30" }),
        "the schedule is kept"
    );
    assert_eq!(updated["action"]["command"], "make report-all");

    let fresh = updated["sha256"].clone();
    for change in [
        json!({ "scope": "global" }),
        json!({ "project": machine.root.temp.path() }),
    ] {
        let arguments = with(json!({ "id": id, "sha256": fresh }), change.clone());
        let refused = machine.refused("routine_update", arguments);
        assert!(refused.contains("ask the user"), "{change}: {refused}");
    }

    let kept = machine.ok("routine_get", json!({ "id": id }));
    assert_eq!(kept["routine"]["sha256"], fresh, "nothing was written");

    let agents = machine.ok("routine_add", add_args())["added"].clone();
    let moved = machine.ok(
        "routine_update",
        json!({ "id": agents["id"], "sha256": agents["sha256"], "scope": "global" }),
    );
    assert_eq!(
        moved["updated"]["scope"], "global",
        "a routine an agent created may move"
    );
    assert_eq!(moved["updated"]["project"], Value::Null);
    assert_eq!(moved["updated"]["status"], "pending");
}

#[test]
fn removal_needs_a_routine_an_agent_created_at_the_version_read() {
    let mut machine = machine();
    let (id, sha256) = machine.desktop_routine(&command_draft("Nightly report"));

    let refused = machine.refused("routine_remove", json!({ "id": id, "sha256": sha256 }));
    assert!(refused.contains("ask them"), "{refused}");
    assert!(machine.root.store.get(&id).is_ok(), "the user's routine is kept");

    let added = machine.ok("routine_add", add_args())["added"].clone();
    let stale = added["sha256"].clone();
    machine.ok(
        "routine_update",
        json!({ "id": added["id"], "sha256": stale, "enabled": false }),
    );

    let conflict = machine.refused("routine_remove", json!({ "id": added["id"], "sha256": stale }));
    assert!(conflict.contains("changed since you read it"), "{conflict}");
    let edit = machine.refused(
        "routine_update",
        json!({ "id": added["id"], "sha256": stale, "name": "X" }),
    );
    assert!(edit.contains("call routine_get"), "{edit}");

    let current = machine.ok("routine_get", json!({ "id": added["id"] }))["routine"]["sha256"].clone();
    let removed = machine.ok("routine_remove", json!({ "id": added["id"], "sha256": current }));
    assert_eq!(removed["removed"], added["id"]);

    let gone = machine.refused("routine_get", json!({ "id": added["id"] }));
    assert_eq!(gone, "no routine has this id");
}

#[test]
fn secrets_in_a_prompt_or_command_are_not_shown_to_the_agent() {
    let mut machine = machine();
    let token = ["gh", "p_", &"Zx9q".repeat(9)].concat();

    let prompt = format!("Call the API with {token} and sum up.");
    let agent = machine.ok("routine_add", with(add_args(), json!({ "prompt": prompt })))["added"].clone();

    let command = with(
        add_args(),
        json!({
            "action": "command",
            "command": format!("curl -H 'token: {token}' https://example.invalid"),
            "harness": null,
            "prompt": null,
        }),
    );
    let shell = machine.ok("routine_add", command)["added"].clone();

    for shown in [
        agent.clone(),
        shell.clone(),
        machine.ok("routine_get", json!({ "id": agent["id"] })),
        machine.ok("routine_list", json!({})),
    ] {
        let text = shown.to_string();
        assert!(!text.contains(&token), "{text}");
        assert!(text.contains("Call the API") || text.contains("curl -H"), "{text}");
    }

    for id in [&agent["id"], &shell["id"]] {
        let written = std::fs::read_to_string(file(&machine.root.store, id.as_str().unwrap())).unwrap();
        assert!(written.contains(&token), "the file keeps what the agent wrote");
    }
}

#[test]
fn invalid_schedules_and_actions_are_refused_with_the_format_wording() {
    let mut machine = machine();

    let cases = [
        (
            json!({ "when": "cron", "at": null, "cron": "0 0 31 2 *" }),
            "never matches",
        ),
        (json!({ "when": "cron", "at": null, "cron": "0 9 * *" }), "five fields"),
        (json!({ "at": "9:00" }), "when.at must be a time like 09:00"),
        (json!({ "at": null }), "when weekdays needs at"),
        (json!({ "when": "everyHours", "at": null, "every": 25 }), "from 1 to 24"),
        (json!({ "every": 5 }), "every belongs to when"),
        (
            json!({ "harness": "pi", "approvalMode": "yolo" }),
            "action.approvalMode is for omp or opencode only",
        ),
        (json!({ "trustOnce": true }), "action.trustOnce is for pi only"),
        (json!({ "maxMinutes": 241 }), "from 1 to 240"),
        (
            json!({ "command": "ls" }),
            "command belongs to the action command, not agent",
        ),
        (json!({ "prompt": null }), "the action agent needs prompt"),
        (json!({ "model": "sonnet" }), "provider/id"),
        (json!({ "name": null }), "missing field `name`"),
        (json!({ "colour": "red" }), "unknown field"),
        (
            json!({ "scope": "global", "action": "script", "harness": null, "prompt": null, "script": "tidy", "scriptScope": "repo" }),
            "use a global script",
        ),
        (
            json!({ "action": "script", "harness": null, "prompt": null, "script": "Tidy Up", "scriptScope": "repo" }),
            "script name",
        ),
    ];

    for (change, expected) in cases {
        let refused = machine.refused("routine_add", with(add_args(), change.clone()));
        assert!(refused.contains(expected), "{change}: {refused}");
    }

    assert!(machine.root.store.list().routines.is_empty(), "nothing was written");
}

#[test]
fn model_is_provider_slash_id_and_an_edit_keeps_what_it_leaves_out() {
    let mut machine = machine();

    let added = machine.ok(
        "routine_add",
        with(
            add_args(),
            json!({ "model": "openrouter/anthropic/claude-sonnet", "approvalMode": "yolo", "maxMinutes": 45 }),
        ),
    )["added"]
        .clone();
    assert_eq!(
        added["action"]["model"],
        json!({ "provider": "openrouter", "id": "anthropic/claude-sonnet" }),
        "the id keeps its slashes"
    );
    assert_eq!(added["action"]["approvalMode"], "yolo");

    let weekly = machine.ok(
        "routine_update",
        json!({ "id": added["id"], "sha256": added["sha256"], "when": "daily" }),
    )["updated"]
        .clone();
    assert_eq!(weekly["when"], json!({ "type": "daily", "at": "09:00" }), "at is kept");
    assert_eq!(weekly["action"], added["action"], "the action is kept");

    let opencode = machine.ok(
        "routine_update",
        json!({ "id": added["id"], "sha256": weekly["sha256"], "harness": "opencode" }),
    )["updated"]
        .clone();
    assert_eq!(opencode["action"]["approvalMode"], "yolo", "opencode keeps it");

    let pi = machine.ok(
        "routine_update",
        json!({ "id": added["id"], "sha256": opencode["sha256"], "harness": "pi", "model": "" }),
    )["updated"]
        .clone();
    assert_eq!(pi["action"]["harness"], "pi");
    assert_eq!(pi["action"]["model"], Value::Null, "an empty model clears it");
    assert_eq!(
        pi["action"]["approvalMode"],
        Value::Null,
        "an omp approval mode is dropped"
    );
    assert_eq!(pi["action"]["maxMinutes"], json!(45));

    let omp = machine.ok(
        "routine_update",
        json!({ "id": added["id"], "sha256": pi["sha256"], "harness": "omp", "approvalMode": "write" }),
    )["updated"]
        .clone();
    let cleared = machine.ok(
        "routine_update",
        json!({ "id": added["id"], "sha256": omp["sha256"], "approvalMode": "default" }),
    );
    assert_eq!(cleared["updated"]["action"]["approvalMode"], Value::Null);

    let nothing = machine.refused(
        "routine_update",
        json!({ "id": added["id"], "sha256": cleared["updated"]["sha256"] }),
    );
    assert!(nothing.contains("at least one field"), "{nothing}");
}

#[test]
fn team_routines_are_never_shown_to_agents() {
    let mut machine = machine();
    let id = rid(9);
    let row = team_row(&id, body(&id, "Team digest", false, command_json()), false);
    machine.root.folder.write_row(&row).unwrap();

    let mut local = command_draft("Local");
    local.action = agent_action(true);
    machine.desktop_routine(&local);

    let listed = machine.ok("routine_list", json!({}));
    assert_eq!(
        listed["routines"].as_array().unwrap().len(),
        1,
        "only the local routine"
    );
    assert!(!listed.to_string().contains(&id), "{listed}");

    let hidden = machine.refused("routine_get", json!({ "id": id }));
    assert_eq!(hidden, "no routine has this id");
}

#[test]
fn without_a_routines_folder_every_routine_tool_refuses() {
    let mut server = Server::new(None, None);
    let result = rpc(
        &mut server,
        "tools/call",
        json!({ "name": "routine_list", "arguments": {} }),
    );

    assert_eq!(result["isError"], json!(true));
    assert!(shown(&result).contains("routines are not available"));
}
