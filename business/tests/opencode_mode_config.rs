//! An approval mode as the chat server's configuration (`mode_config`), proven against a port of
//! what OpenCode does with it: the user's configuration merged with ours (`mergeDeep`), each
//! agent's `permission` turned into rules (`Permission.fromConfig`) after its own, and the last
//! matching rule deciding. The recorded agents (`fixtures/opencode/server/agent-list.json`) stand
//! for OpenCode's built-in and global rules, with user agent permissions appended as OpenCode does.

mod opencode_mapper_support;

use harness_business::model::ApprovalMode;
use harness_business::opencode_server::mode_config::{
    check_applied, config_rules, inherited_config, merge_deep, mode_config, permission_object, stored_overrides,
};
use harness_business::opencode_server::modes::{Rule, agent_rules, evaluate, mode_rules};
use opencode_mapper_support::recorded_body;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const MODES: [ApprovalMode; 3] = [ApprovalMode::AlwaysAsk, ApprovalMode::Write, ApprovalMode::Yolo];

const PERMISSIONS: [&str; 22] = [
    "bash",
    "edit",
    "write",
    "apply_patch",
    "read",
    "glob",
    "grep",
    "list",
    "lsp",
    "skill",
    "todowrite",
    "question",
    "plan_enter",
    "plan_exit",
    "plan_deploy",
    "task",
    "webfetch",
    "websearch",
    "doom_loop",
    "external_directory",
    "harness-memory_script_run",
    "foo_bar",
];

const PATTERNS: [&str; 11] = [
    "*",
    "src/main.rs",
    ".env",
    "a.env.example",
    ".opencode/plans/next.md",
    "general",
    "explore",
    "git status",
    "ls",
    "/elsewhere/x",
    "rm -rf /",
];

/// OpenCode's decision: the last matching rule, `ask` without one.
fn decide(rules: &[Rule], permission: &str, pattern: &str) -> String {
    evaluate(rules, permission, pattern).map_or_else(|| "ask".to_owned(), |rule| rule.action.clone())
}

/// The recorded agents with the user's agent permissions `user` (`{name: permission}`) applied the
/// way OpenCode applies configuration: after the agent's rules, before its tool-output rule (the
/// recorded lists end with it).
fn agents_with(user: &Value) -> Value {
    let mut agents = recorded_body("agent-list.json");

    for agent in agents.as_array_mut().expect("agents") {
        let name = agent["name"].as_str().expect("name").to_owned();
        let mut rules: Vec<Rule> = serde_json::from_value(agent["permission"].clone()).expect("rules");
        let last = rules.pop().expect("the tool-output rule");
        rules.extend(config_rules(&user[&name], None));
        rules.push(last);
        agent["permission"] = json!(rules);
    }

    agents
}

/// The agents a server started with `content` reports: the recorded ones with the content's agent
/// permissions applied.
fn served(content: &Value) -> Value {
    let user: serde_json::Map<String, Value> = content["agent"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(name, agent)| (name.clone(), agent["permission"].clone()))
        .collect();

    agents_with(&Value::Object(user))
}

/// The configuration that holds the user's agent permissions `user` (`{name: permission}`).
fn user_config(user: &Value) -> Value {
    let agents = user.as_object().into_iter().flatten();
    let agents: serde_json::Map<String, Value> = agents
        .map(|(name, permission)| (name.clone(), json!({ "permission": permission })))
        .collect();

    json!({ "agent": agents })
}

/// Checks, for every agent and call, that the server started with the mode decides as the mode's
/// rules for the agent without it, except OpenCode's own tool-output folder.
fn assert_exact(mode: ApprovalMode, user: &Value) {
    let user_config = user_config(user);
    let probed = agents_with(user);
    let config = mode_config(mode, &probed).expect("expressible");
    let content = merge_deep(&user_config, &config);
    let applied = served(&content);

    check_applied(&probed, &applied, &config).expect("applied as set");

    for agent in probed.as_array().expect("agents") {
        let name = agent["name"].as_str().expect("name");
        let own = agent_rules(&probed, name);
        let intended = mode_rules(Some(mode), &own).expect("rules");
        let actual = agent_rules(&applied, name);

        for permission in PERMISSIONS {
            for pattern in PATTERNS {
                assert_eq!(
                    decide(&actual, permission, pattern),
                    decide(&intended, permission, pattern),
                    "{mode:?} {name} {permission} {pattern} with {user}"
                );
            }
        }
    }
}

#[test]
fn every_agent_decides_as_the_mode_says() {
    for mode in MODES {
        assert_exact(mode, &json!({}));
    }
}

#[test]
fn the_users_own_agent_permissions_stay_before_the_mode() {
    let users = [
        json!({ "build": { "bash": "allow", "*": "ask", "edit": { "*.md": "allow", "src/*": "deny" } } }),
        json!({ "plan": { "edit": "deny" }, "general": { "read": { "*.env": "allow" }, "webfetch": "deny" } }),
        json!({ "explore": { "*": "allow", "bash": { "git *": "allow", "*": "deny" } } }),
        json!({ "build": { "plan_*": "deny", "task": { "explore": "deny" }, "harness-memory_*": "allow" } }),
    ];

    for user in users {
        for mode in MODES {
            assert_exact(mode, &user);
        }
    }
}

#[test]
fn the_plan_agent_stays_read_only_and_subagents_follow_the_mode() {
    let probed = agents_with(&json!({}));
    let rules_of = |mode: ApprovalMode, agent: &str| {
        let config = mode_config(mode, &probed).expect("expressible");
        agent_rules(&served(&config), agent)
    };

    let plan = rules_of(ApprovalMode::Yolo, "plan");
    assert_eq!(
        decide(&plan, "edit", "src/main.rs"),
        "deny",
        "a command with agent plan cannot edit"
    );
    assert_eq!(decide(&plan, "bash", "ls"), "allow");

    let general = rules_of(ApprovalMode::Write, "general");
    assert_eq!(decide(&general, "bash", "ls"), "ask", "a subagent asks in Write");
    assert_eq!(decide(&general, "edit", "src/main.rs"), "ask");
    assert_eq!(decide(&general, "read", "src/main.rs"), "allow");

    let explore = rules_of(ApprovalMode::AlwaysAsk, "explore");
    assert_eq!(decide(&explore, "grep", "fn"), "ask");
    assert_eq!(decide(&explore, "edit", "src/main.rs"), "deny", "explore's deny stays");
}

#[test]
fn keys_are_variants_the_user_cannot_have() {
    let rules = [
        Rule::new("*", "*", "ask"),
        Rule::new("*", "*", "allow"),
        Rule::new("read", "*", "allow"),
        Rule::new("plan_*", "*", "deny"),
        Rule::new("read", "*.env", "ask"),
        Rule::new("*", "*", "deny"),
        Rule::new("read", "*", "allow"),
    ];

    let object = permission_object(&rules).expect("expressible");
    let keys: Vec<&str> = object.keys().map(String::as_str).collect();
    assert_eq!(keys, ["* *", "** *", "plan_* *", "*** *", "read *"]);
    assert_eq!(
        object["read *"],
        json!({ "*.env": "ask", "*": "allow" }),
        "every rule that applies to read, a pattern at its last place"
    );

    for permission in ["read", "bash", "plan_exit", "reader"] {
        for pattern in ["x", ".env"] {
            let from_config = config_rules(&Value::Object(object.clone()), None);
            assert_eq!(
                decide(&from_config, permission, pattern),
                decide(&rules, permission, pattern),
                "{permission} {pattern}"
            );
        }
    }
}

#[test]
fn a_layer_that_changes_the_agents_after_ours_is_caught() {
    let probed = agents_with(&json!({}));
    let config = mode_config(ApprovalMode::Yolo, &probed).expect("expressible");

    assert!(check_applied(&probed, &probed, &config).is_err(), "the mode is missing");
    check_applied(&probed, &served(&config), &config).expect("as set");

    let mut later = config.clone();
    later["agent"]["plan"]["permission"]["edit"] = json!("allow");
    assert!(
        check_applied(&probed, &served(&later), &config).is_err(),
        "a rule after ours"
    );

    let user = json!({ "agent": { "build": { "permission": { "read *": "deny" } } } });
    let colliding = merge_deep(&user, &config);
    let probed_with_user = agents_with(&json!({ "build": { "read *": "deny" } }));
    assert!(
        check_applied(&probed_with_user, &served(&colliding), &config).is_err(),
        "a user key like ours moves our rules"
    );
}

#[test]
fn merge_deep_keeps_the_targets_order_and_takes_the_sources_values() {
    let merged = merge_deep(
        &json!({ "a": { "x": 1, "y": 2 }, "b": "keep", "c": [1] }),
        &json!({ "a": { "y": 3, "z": 4 }, "c": [2], "d": true }),
    );

    assert_eq!(
        merged.to_string(),
        r#"{"a":{"x":1,"y":3,"z":4},"b":"keep","c":[2],"d":true}"#
    );
    assert_eq!(
        merge_deep(&json!({ "p": { "x": "allow" } }), &json!({ "p": "ask" })),
        json!({ "p": "ask" })
    );
}

#[test]
fn the_inherited_content_must_be_an_object() {
    assert_eq!(inherited_config(None), Ok(json!({})));
    assert_eq!(inherited_config(Some("  ")), Ok(json!({})));
    assert_eq!(
        inherited_config(Some("{ // mine\n \"share\": \"disabled\", }")),
        Ok(json!({ "share": "disabled" }))
    );
    assert!(inherited_config(Some("{ \"model\": {env:MODEL} }")).is_err());
    assert!(inherited_config(Some("[1]")).is_err());
}

#[test]
fn from_config_reads_strings_objects_and_home_patterns() {
    let home = std::path::Path::new("/home/me");
    let rules = config_rules(
        &json!({ "bash": "ask", "external_directory": { "~/notes/*": "allow", "$HOME/x": "deny" } }),
        Some(home),
    );

    assert_eq!(
        rules,
        [
            Rule::new("bash", "*", "ask"),
            Rule::new("external_directory", "/home/me/notes/*", "allow"),
            Rule::new("external_directory", "/home/me/x", "deny"),
        ]
    );
    assert_eq!(config_rules(&json!("deny"), None), [Rule::new("*", "*", "deny")]);
}

/// The applied agents and the check's verdict for user agent permissions `user` in `mode`.
fn checked(mode: ApprovalMode, user: &Value) -> Result<(), String> {
    let user_config = user_config(user);
    let probed = agents_with(user);
    let config = mode_config(mode, &probed)?;

    check_applied(&probed, &served(&merge_deep(&user_config, &config)), &config)
}

#[test]
fn an_agent_permission_given_as_one_string_refuses_the_mode() {
    // `mergeDeep` replaces the string with our object: the agent's own rules would change.
    let refused = checked(ApprovalMode::Yolo, &json!({ "plan": "deny" }));
    assert!(refused.is_err_and(|why| why.ends_with("the plan agent's permissions")));
}

#[test]
fn agents_that_differ_from_the_probes_refuse_the_mode() {
    let project = agents_with(&json!({}));
    let config = mode_config(ApprovalMode::Yolo, &project).expect("expressible");

    // A server working in a folder of the project whose own configuration denies `rm`.
    let folder = agents_with(&json!({ "build": { "bash": { "rm *": "deny" } } }));
    assert!(check_applied(&project, &served_on(&folder, &config), &config).is_err());

    let mut renamed = served(&config);
    renamed.as_array_mut().expect("agents").swap(0, 1);
    assert!(
        check_applied(&project, &renamed, &config).is_err(),
        "another agent order"
    );
}

/// `agents` with `content`'s agent permissions applied like OpenCode.
fn served_on(agents: &Value, content: &Value) -> Value {
    let mut served = agents.clone();

    for agent in served.as_array_mut().expect("agents") {
        let name = agent["name"].as_str().expect("name").to_owned();
        let mut rules: Vec<Rule> = serde_json::from_value(agent["permission"].clone()).expect("rules");
        let last = rules.pop().expect("the tool-output rule");
        rules.extend(config_rules(&content["agent"][&name]["permission"], None));
        rules.push(last);
        agent["permission"] = json!(rules);
    }

    served
}

#[test]
fn agent_lists_the_mode_cannot_trust_are_refused() {
    let named =
        |name: &str| json!({ "name": name, "permission": [{ "permission": "*", "pattern": "*", "action": "allow" }] });

    for agents in [
        json!([]),
        json!({ "build": {} }),
        json!([{ "permission": [] }]),
        json!([named("build"), named("build")]),
        json!([{ "name": "build", "permission": "allow" }]),
    ] {
        assert!(mode_config(ApprovalMode::Write, &agents).is_err(), "{agents}");
    }

    let config = mode_config(ApprovalMode::Write, &json!([named("build")])).expect("one agent");
    assert!(
        check_applied(&json!([]), &json!([]), &config).is_err(),
        "nothing probed"
    );

    let twice = json!([named("build"), named("build")]);
    assert!(check_applied(&twice, &twice, &config).is_err(), "a name listed twice");
}

#[test]
fn keys_that_would_collide_or_reorder_are_refused() {
    let colliding = [
        Rule::new("*", "*", "ask"),
        Rule::new("*", "*", "allow"),
        Rule::new("**", "*", "deny"),
    ];
    assert_eq!(
        permission_object(&colliding),
        Err("two permission keys come out as \"** *\"".to_owned())
    );

    let numbered = [Rule::new("*", "*", "ask"), Rule::new("bash", "1", "deny")];
    assert!(
        permission_object(&numbered).is_err(),
        "JavaScript would move a number key first"
    );
    assert!(permission_object(&[Rule::new("bash", "01", "deny"), Rule::new("*", "*", "ask")]).is_ok());

    let question = [Rule::new("rea?", "*", "ask"), Rule::new("rea?", "*", "allow")];
    assert!(permission_object(&question).is_err(), "a ? key cannot repeat");
}

#[test]
fn stored_session_rules_other_than_denies_override_a_mode() {
    assert!(!stored_overrides(&Value::Null));
    assert!(!stored_overrides(&json!([])));
    assert!(!stored_overrides(
        &json!([{ "permission": "bash", "pattern": "*", "action": "deny" }])
    ));
    assert!(stored_overrides(
        &json!([{ "permission": "*", "pattern": "*", "action": "allow" }])
    ));
    assert!(stored_overrides(
        &json!([{ "permission": "edit", "pattern": "*", "action": "ask" }])
    ));
    assert!(stored_overrides(&json!("odd")));
}

#[test]
fn rules_of_one_action_may_come_in_another_order_on_the_second_start() {
    // OpenCode adds an `allow` per skill folder it found, in another order on every start.
    let user = json!({ "build": { "external_directory": { "/a/*": "allow", "/b/*": "allow", "/c/*": "deny" } } });
    let probed = agents_with(&user);
    let config = mode_config(ApprovalMode::Write, &probed).expect("expressible");
    let applied = served(&merge_deep(&user_config(&user), &config));
    check_applied(&probed, &applied, &config).expect("as set");

    let swapped = |first: &str, second: &str| {
        let mut agents = applied.clone();
        let build = agents
            .as_array_mut()
            .expect("agents")
            .iter_mut()
            .find(|agent| agent["name"] == "build")
            .expect("build");
        let rules = build["permission"].as_array_mut().expect("rules");
        let at = |pattern: &str| {
            rules
                .iter()
                .position(|rule| rule["permission"] == "external_directory" && rule["pattern"] == pattern)
                .expect("the user's rule")
        };
        let (a, b) = (at(first), at(second));
        rules.swap(a, b);
        agents
    };

    check_applied(&probed, &swapped("/a/*", "/b/*"), &config).expect("two allows swapped decide the same");
    assert!(
        check_applied(&probed, &swapped("/b/*", "/c/*"), &config).is_err(),
        "an allow and a deny swapped decide otherwise"
    );
}

#[test]
fn only_opencodes_tool_output_rule_may_follow_the_mode() {
    let probed = json!([{ "name": "build", "permission": [
        { "permission": "*", "pattern": "*", "action": "allow" },
        { "permission": "external_directory", "pattern": "/x/*", "action": "allow" },
    ] }]);
    let config = mode_config(ApprovalMode::AlwaysAsk, &probed).expect("expressible");
    let mut rules: Vec<Rule> = serde_json::from_value(probed[0]["permission"].clone()).expect("rules");
    let moved = rules.pop().expect("the folder rule");
    rules.extend(config_rules(&config["agent"]["build"]["permission"], None));
    rules.push(moved);
    let applied = json!([{ "name": "build", "permission": rules }]);

    assert!(
        check_applied(&probed, &applied, &config).is_err(),
        "a folder allow after the mode is not OpenCode's tool-output rule"
    );
}
