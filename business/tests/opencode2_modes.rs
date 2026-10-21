//! Approval modes as OpenCode 2 permission rules and as the configuration of the chat's own server,
//! against the agents OpenCode 2.1.2 listed (`fixtures/opencode2/recorded/ops.jsonl`): what each
//! mode decides, the configuration, an inherited `OPENCODE_CONFIG_CONTENT`, the check of the
//! server started with it, and the session and project rules that would override a mode.

mod opencode2_recorded;

use harness_business::model::ApprovalMode;
use harness_business::opencode_server::http::Method;
use harness_business::opencode_server_v2::RouteV2;
use harness_business::opencode_server_v2::mode_config::{
    check_applied, configured_rules, listed_agents, merge_content, mode_config, saved_overrides, stored_overrides,
};
use harness_business::opencode_server_v2::modes::{Rule, decide, deny_all, mode_rules};
use harness_business::opencode_server_v2::wire::CreateBody;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The agents OpenCode 2.1.2 listed (`GET /api/agent` `data`), each ending with the `browser` deny
/// its browser connection adds.
fn recorded() -> Value {
    let listed = opencode2_recorded::find("GET", "/api/agent");

    listed[0].response["data"].clone()
}

fn rules_of(agents: &Value, id: &str) -> Vec<Rule> {
    let listed = listed_agents(agents).expect("agents");

    listed
        .into_iter()
        .find(|(agent, _)| agent == id)
        .map(|(_, rules)| rules)
        .expect("listed")
}

/// The agents as OpenCode lists them once it applied `config`: each agent's rules, the mode's,
/// then the `browser` deny that comes last.
fn applied(agents: &Value, config: &Value) -> Value {
    let mut agents = agents.clone();

    for agent in agents.as_array_mut().expect("list") {
        let id = agent["id"].as_str().expect("id").to_owned();
        let rules = agent["permissions"].as_array_mut().expect("rules");
        let browser = rules.pop().expect("the browser deny");

        rules.extend(
            config["agents"][&id]["permissions"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
        );
        rules.push(browser);
    }

    agents
}

/// What agent `id` decides for `action` on `resource` with `mode`.
fn decision(mode: ApprovalMode, id: &str, action: &str, resource: &str) -> String {
    let agents = recorded();
    let config = mode_config(mode, &agents).expect("config");

    decide(&rules_of(&applied(&agents, &config), id), action, resource).to_owned()
}

#[test]
fn always_ask_asks_before_everything_but_questions_and_todos() {
    let ask = |id, action, resource| decision(ApprovalMode::AlwaysAsk, id, action, resource);

    for action in [
        "shell",
        "edit",
        "read",
        "glob",
        "webfetch",
        "subagent",
        "skill",
        "my_mcp_tool",
    ] {
        assert_eq!(ask("build", action, "x"), "ask", "{action}");
    }

    assert_eq!(ask("build", "question", "*"), "allow");
    assert_eq!(ask("build", "todowrite", "*"), "allow");
    assert_eq!(ask("explore", "shell", "ls"), "ask", "subagents ask too");
    assert_eq!(
        ask("plan", "edit", "src/main.rs"),
        "deny",
        "the plan agent stays read-only"
    );
    assert_eq!(ask("general", "question", "*"), "deny", "the agent's denies stay");
    assert_eq!(ask("title", "read", "a.txt"), "deny");
    assert_eq!(ask("build", "browser", "x"), "deny");
}

#[test]
fn write_lets_reads_through_and_asks_before_the_rest() {
    let write = |id, action, resource| decision(ApprovalMode::Write, id, action, resource);

    for action in [
        "read",
        "glob",
        "grep",
        "skill",
        "todowrite",
        "question",
        "opencode_list_mcp_resources",
        "opencode_read_mcp_resource",
    ] {
        assert_eq!(write("build", action, "x"), "allow", "{action}");
    }

    for action in [
        "edit",
        "shell",
        "subagent",
        "webfetch",
        "websearch",
        "my_mcp_tool",
        "external_directory",
    ] {
        assert_eq!(write("build", action, "x"), "ask", "{action}");
    }

    assert_eq!(write("build", "read", "app/.env"), "ask", "the agent's own asks stay");
    assert_eq!(
        write("explore", "external_directory", "/tmp/x"),
        "allow",
        "kept as the agent has it"
    );
    assert_eq!(write("explore", "edit", "a.txt"), "deny");
    assert_eq!(write("plan", "edit", "src/main.rs"), "deny");
}

#[test]
fn yolo_allows_what_asks_and_keeps_the_denies() {
    let yolo = |id, action, resource| decision(ApprovalMode::Yolo, id, action, resource);

    assert_eq!(yolo("build", "shell", "rm -r build"), "allow");
    assert_eq!(yolo("build", "read", "app/.env"), "allow");
    assert_eq!(yolo("explore", "read", ".env"), "allow");
    assert_eq!(yolo("plan", "edit", "src/main.rs"), "deny");
    assert_eq!(yolo("general", "subagent", "explore"), "deny");
    assert_eq!(yolo("summary", "shell", "ls"), "deny");
    assert_eq!(yolo("build", "browser", "x"), "deny");
}

#[test]
fn no_override_sets_no_rules_and_a_helper_chat_denies_everything() {
    assert_eq!(mode_rules(None, &rules_of(&recorded(), "build")), None);
    assert_eq!(deny_all(), vec![Rule::new("*", "*", "deny")]);
    assert_eq!(decide(&deny_all(), "question", "*"), "deny");
}

#[test]
fn the_configuration_names_every_listed_agent_once() {
    let agents = recorded();
    let config = mode_config(ApprovalMode::Write, &agents).expect("config");

    let mut configured: Vec<&String> = config["agents"].as_object().expect("agents").keys().collect();
    configured.sort();
    assert_eq!(
        configured,
        ["build", "compaction", "explore", "general", "plan", "summary", "title"]
    );

    let build = configured_rules(&config, "build");
    assert_eq!(build[0], Rule::new("*", "*", "ask"), "a leading catch-all");
    assert_eq!(
        config["agents"]["build"].as_object().expect("build").len(),
        1,
        "rules only"
    );

    let refused = |agents: Value| mode_config(ApprovalMode::Yolo, &agents).expect_err("refused");
    assert_eq!(refused(json!([])), "OpenCode listed no agents");
    assert_eq!(refused(json!({})), "OpenCode listed no agents");
    assert_eq!(
        refused(json!([{ "name": "Build", "permissions": [] }])),
        "OpenCode listed an agent without an id"
    );
    assert_eq!(
        refused(json!([{ "id": "build", "permissions": [] }, { "id": "build", "permissions": [] }])),
        "two agents have the id build"
    );
    assert_eq!(
        refused(json!([{ "id": "build", "permissions": "allow" }])),
        "the build agent's permissions cannot be read"
    );
}

#[test]
fn an_inherited_configuration_keeps_its_settings_and_rules_first() {
    let config = mode_config(ApprovalMode::Write, &recorded()).expect("config");
    let mine = json!({
        "share": "disabled",
        "permissions": [{ "action": "webfetch", "resource": "*", "effect": "deny" }],
        "agents": { "build": { "description": "Mine", "permissions": [{ "action": "shell", "resource": "git push*", "effect": "deny" }] } },
    });

    let merged = merge_content(&mine, &config).expect("merged");
    assert_eq!(merged["share"], "disabled");
    assert_eq!(merged["permissions"], mine["permissions"]);
    assert_eq!(merged["agents"]["build"]["description"], "Mine");

    let build: Vec<Rule> = serde_json::from_value(merged["agents"]["build"]["permissions"].clone()).expect("rules");
    assert_eq!(
        build[0],
        Rule::new("shell", "git push*", "deny"),
        "the inherited rule first"
    );
    assert_eq!(build[1..], configured_rules(&config, "build")[..]);
    assert_eq!(
        merged["agents"]["plan"]["permissions"],
        config["agents"]["plan"]["permissions"]
    );

    for (legacy, key) in [
        (
            json!({ "agent": { "build": { "permission": { "bash": "allow" } } } }),
            "agent.build",
        ),
        (json!({ "mode": { "plan": { "permission": "ask" } } }), "mode.plan"),
    ] {
        let refused = merge_content(&legacy, &config).expect_err("refused");
        assert!(refused.contains(key), "{refused}");
    }

    let odd = json!({ "agents": { "build": { "permissions": "allow" } } });
    let refused = merge_content(&odd, &config).expect_err("refused");
    assert_eq!(refused, "its build agent's permissions are not a list");
}

#[test]
fn the_started_server_must_show_the_mode_after_the_probes_rules() {
    let agents = recorded();
    let config = mode_config(ApprovalMode::AlwaysAsk, &agents).expect("config");

    check_applied(&agents, &applied(&agents, &config), &config).expect("as OpenCode applies it");

    // The probe read the agents before the browser deny came: it follows the mode's rules.
    let mut early = agents.clone();
    for agent in early.as_array_mut().expect("list") {
        agent["permissions"].as_array_mut().expect("rules").pop();
    }
    let config = mode_config(ApprovalMode::AlwaysAsk, &early).expect("config");
    check_applied(&early, &applied(&agents, &config), &config).expect("a later deny only refuses more");

    // The chat's server listed the agents before the browser deny came.
    let config = mode_config(ApprovalMode::AlwaysAsk, &agents).expect("config");
    let mut first = applied(&agents, &config);
    for agent in first.as_array_mut().expect("list") {
        agent["permissions"].as_array_mut().expect("rules").pop();
    }
    check_applied(&agents, &first, &config).expect("the probe's denies are in the mode's rules");
}

#[test]
fn rules_of_one_effect_may_come_in_another_order() {
    let probe = json!([{ "id": "build", "permissions": [
        { "action": "*", "resource": "*", "effect": "allow" },
        { "action": "skill", "resource": "/a/*", "effect": "allow" },
        { "action": "skill", "resource": "/b/*", "effect": "allow" },
        { "action": "read", "resource": "*.env", "effect": "ask" },
    ] }]);
    let config = mode_config(ApprovalMode::Yolo, &probe).expect("config");

    let mut reordered = probe.clone();
    reordered[0]["permissions"].as_array_mut().expect("rules").swap(1, 2);
    let ours = config["agents"]["build"]["permissions"]
        .as_array()
        .cloned()
        .expect("ours");
    reordered[0]["permissions"].as_array_mut().expect("rules").extend(ours);

    check_applied(&probe, &reordered, &config).expect("only the order inside a run differs");
}

#[test]
fn a_server_that_changes_the_mode_refuses_it() {
    let agents = recorded();
    let config = mode_config(ApprovalMode::Write, &agents).expect("config");
    let changed = "a managed setting or a plugin changes the build agent's permissions";
    let build = |listed: &mut Value| -> Vec<Value> { listed[0]["permissions"].as_array().cloned().expect("rules") };

    // An allow after the mode's rules (a managed setting, a plugin).
    let mut later = applied(&agents, &config);
    let mut rules = build(&mut later);
    rules.push(json!({ "action": "shell", "resource": "*", "effect": "allow" }));
    later[0]["permissions"] = Value::Array(rules);
    assert_eq!(check_applied(&agents, &later, &config), Err(changed.to_owned()));

    // The mode's rules are not there.
    assert_eq!(check_applied(&agents, &agents, &config), Err(changed.to_owned()));

    // A deny the probe did not see, before the mode's rules: the mode does not keep it.
    for at in [1, 5] {
        let mut extra = applied(&agents, &config);
        let mut rules = build(&mut extra);
        rules.insert(
            at,
            json!({ "action": "shell", "resource": "git push*", "effect": "deny" }),
        );
        extra[0]["permissions"] = Value::Array(rules);
        assert_eq!(
            check_applied(&agents, &extra, &config),
            Err(changed.to_owned()),
            "at {at}"
        );
    }

    // Another agent list.
    let mut more = applied(&agents, &config);
    more.as_array_mut()
        .expect("list")
        .push(json!({ "id": "mine", "permissions": [] }));
    assert_eq!(
        check_applied(&agents, &more, &config),
        Err("the agents differ between the two starts".to_owned())
    );
    assert_eq!(
        check_applied(&agents, &json!([]), &config),
        Err("OpenCode listed no agents".to_owned())
    );
}

#[test]
fn session_rules_other_than_denies_override_a_mode() {
    assert!(!stored_overrides(&Value::Null));
    assert!(!stored_overrides(&json!([])));
    assert!(!stored_overrides(
        &json!([{ "action": "*", "resource": "*", "effect": "deny" }])
    ));
    assert!(stored_overrides(
        &json!([{ "action": "shell", "resource": "*", "effect": "allow" }])
    ));
    assert!(stored_overrides(
        &json!([{ "action": "edit", "resource": "*", "effect": "ask" }])
    ));
    assert!(
        stored_overrides(&json!({ "edit": "allow" })),
        "anything unreadable counts"
    );
}

#[test]
fn saved_rules_override_the_modes_that_ask() {
    let saved = json!([
        { "id": "psv_1", "projectID": "global", "action": "shell", "resource": "ls *" },
        { "id": "psv_2", "projectID": "global", "action": "read", "resource": "*" },
        { "id": "psv_3", "projectID": "global", "action": "shell", "resource": "git *" },
        { "id": "psv_4", "projectID": "global", "action": "question", "resource": "*" },
    ]);

    assert_eq!(saved_overrides(ApprovalMode::AlwaysAsk, &saved), ["shell", "read"]);
    assert_eq!(saved_overrides(ApprovalMode::Write, &saved), ["shell"]);
    assert!(saved_overrides(ApprovalMode::Yolo, &saved).is_empty());
    assert!(saved_overrides(ApprovalMode::AlwaysAsk, &json!([])).is_empty());
}

#[test]
fn saved_rules_are_read_by_project_and_a_helper_session_is_created_with_its_rules() {
    let route = RouteV2::PermissionSaved { project: "global" };
    assert_eq!(route.method(), Method::Get);
    assert_eq!(route.target(), "/api/permission/saved?projectID=global");
    assert_eq!(
        RouteV2::PermissionSaved { project: "a&b c" }.target(),
        "/api/permission/saved?projectID=a%26b%20c"
    );

    let helper = CreateBody {
        permissions: Some(deny_all()),
        ..CreateBody::default()
    };
    assert_eq!(
        serde_json::to_value(helper).expect("body"),
        json!({ "permissions": [{ "action": "*", "resource": "*", "effect": "deny" }] })
    );
    assert_eq!(serde_json::to_value(CreateBody::default()).expect("body"), json!({}));
}
