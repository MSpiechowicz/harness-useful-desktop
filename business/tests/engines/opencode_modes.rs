//! Approval modes as OpenCode permission rules (`modes`), against the recorded `build` and `plan`
//! agents (`fixtures/opencode/server/agent-list.json`): the rules are decided the way OpenCode
//! decides them, the last matching rule of the agent's own rules followed by the mode's.

use crate::opencode_mapper_support;

use harness_business::model::ApprovalMode;
use harness_business::opencode_server::modes::{Rule, agent_rules, deny_all, evaluate, matches, mode_rules};
use opencode_mapper_support::recorded_body;
use pretty_assertions::assert_eq;

/// What OpenCode decides for `permission` on `pattern` (`permission/index.ts` `evaluate`): the
/// last matching rule of the agent's rules, then the mode's; `ask` when none matches.
fn decide(agent: &[Rule], mode: &[Rule], permission: &str, pattern: &str) -> String {
    let rules: Vec<Rule> = agent.iter().chain(mode).cloned().collect();

    evaluate(&rules, permission, pattern).map_or_else(|| "ask".to_owned(), |rule| rule.action.clone())
}

fn recorded(agent: &str) -> Vec<Rule> {
    agent_rules(&recorded_body("agent-list.json"), agent)
}

/// The decisions for `calls` (`permission pattern`) of `agent` in `mode`.
fn decisions(agent: &str, mode: Option<ApprovalMode>, calls: &[(&str, &str)]) -> Vec<String> {
    let rules = agent_rules(&recorded_body("agent-list.json"), agent);
    assert!(!rules.is_empty(), "the recorded {agent} agent has rules");
    let session = mode_rules(mode, &rules).unwrap_or_default();

    calls
        .iter()
        .map(|(permission, pattern)| decide(&rules, &session, permission, pattern))
        .collect()
}

const CALLS: [(&str, &str); 10] = [
    ("bash", "cargo test"),
    ("edit", "src/main.rs"),
    ("edit", ".opencode/plans/next.md"),
    ("read", "src/main.rs"),
    ("read", ".env"),
    ("task", "general"),
    ("question", "*"),
    ("todowrite", "*"),
    ("webfetch", "https://example.com"),
    ("external_directory", "/elsewhere/*"),
];

fn expect(actions: [&str; 10]) -> Vec<String> {
    actions.map(str::to_owned).to_vec()
}

#[test]
fn without_an_override_the_agents_own_rules_apply() {
    assert_eq!(mode_rules(None, &[Rule::new("*", "*", "allow")]), None);

    assert_eq!(
        decisions("build", None, &CALLS),
        expect([
            "allow", "allow", "allow", "allow", "ask", "allow", "allow", "allow", "allow", "ask"
        ])
    );
    assert_eq!(
        decisions("plan", None, &CALLS),
        expect([
            "allow", "deny", "allow", "allow", "ask", "deny", "allow", "allow", "allow", "ask"
        ])
    );
}

#[test]
fn always_ask_asks_for_everything_but_questions_todos_and_plan_switches() {
    let mode = Some(ApprovalMode::AlwaysAsk);

    assert_eq!(
        decisions("build", mode, &CALLS),
        expect(["ask", "ask", "ask", "ask", "ask", "ask", "allow", "allow", "ask", "ask"])
    );
    assert_eq!(
        decisions("plan", mode, &CALLS),
        expect([
            "ask", "deny", "ask", "ask", "ask", "deny", "allow", "allow", "ask", "ask"
        ]),
        "the plan agent's denies stay"
    );
}

#[test]
fn write_asks_for_everything_but_reading() {
    let mode = Some(ApprovalMode::Write);

    assert_eq!(
        decisions("build", mode, &CALLS),
        expect([
            "ask", "ask", "ask", "allow", "ask", "ask", "allow", "allow", "ask", "ask"
        ])
    );
    assert_eq!(
        decisions("plan", mode, &CALLS),
        expect([
            "ask", "deny", "ask", "allow", "ask", "deny", "allow", "allow", "ask", "ask"
        ]),
        "the plan agent's denies stay"
    );

    let tools = [
        ("harness-memory_script_run", "*"),
        ("foo_bar", "*"),
        ("webfetch", "https://example.com"),
        ("websearch", "rust"),
        ("apply_patch", "src/lib.rs"),
        ("edit", "src/lib.rs"),
        ("bash", "ls"),
        ("task", "general"),
        ("read", "src/lib.rs"),
        ("glob", "**/*.rs"),
        ("grep", "fn main"),
        ("external_directory", "/work/tmp/opencode/x"),
        ("plan_deploy", "*"),
        ("plan_enter", "*"),
    ];
    assert_eq!(
        decisions("build", mode, &tools),
        [
            "ask", "ask", "ask", "ask", "ask", "ask", "ask", "ask", "allow", "allow", "allow", "allow", "ask", "allow"
        ],
        "MCP (a server named plan too), plugin and web tools ask, reads, plan switches and the agent's \
         allowed folders outside do not"
    );
}

#[test]
fn yolo_allows_everything_but_keeps_the_agents_denies() {
    let mode = Some(ApprovalMode::Yolo);

    assert_eq!(
        decisions("build", mode, &CALLS),
        expect([
            "allow", "allow", "allow", "allow", "allow", "allow", "allow", "allow", "allow", "allow"
        ])
    );
    assert_eq!(
        decisions("plan", mode, &CALLS),
        expect([
            "allow", "deny", "allow", "allow", "allow", "deny", "allow", "allow", "allow", "allow"
        ]),
        "the plan agent stays read-only"
    );

    let session = mode_rules(mode, &[]).expect("rules");
    assert_eq!(session, [Rule::new("*", "*", "allow")]);
}

#[test]
fn yolo_keeps_asking_where_the_agent_denies_and_the_user_asks() {
    let user_asks_for_edits = Rule::new("edit", "*", "ask");
    let mut plan = recorded("plan");
    plan.push(user_asks_for_edits.clone());
    let mut build = recorded("build");
    build.push(user_asks_for_edits);

    let yolo = |agent: &[Rule]| mode_rules(Some(ApprovalMode::Yolo), agent).expect("rules");
    assert_eq!(
        decide(&plan, &yolo(&plan), "edit", "src/main.rs"),
        "ask",
        "plan never edits unasked"
    );
    assert_eq!(decide(&build, &yolo(&build), "edit", "src/main.rs"), "allow");

    let mut asks_everything = recorded("plan");
    asks_everything.push(Rule::new("*", "*", "ask"));
    let session = yolo(&asks_everything);
    assert_eq!(decide(&asks_everything, &session, "edit", "src/main.rs"), "ask");
    assert_eq!(decide(&asks_everything, &session, "bash", "ls"), "allow");
}

#[test]
fn a_helper_chat_denies_every_tool() {
    let build = agent_rules(&recorded_body("agent-list.json"), "build");
    let denied: Vec<String> = CALLS
        .iter()
        .map(|(permission, pattern)| decide(&build, &deny_all(), permission, pattern))
        .collect();

    assert_eq!(denied, vec!["deny".to_owned(); CALLS.len()]);
}

#[test]
fn wildcards_match_like_opencode() {
    assert!(matches("plan_enter", "plan_*"));
    assert!(matches("plan_*", "*"));
    assert!(matches(".opencode/plans/next.md", ".opencode/plans/*.md"));
    assert!(matches("a.env", "*.env"));
    assert!(!matches("a.env.example", "*.env"));
    assert!(matches("ab", "a?"));
    assert!(!matches("edit", "bash"));
    assert!(agent_rules(&recorded_body("agent-list.json"), "nobody").is_empty());
}

#[test]
fn wildcards_follow_opencodes_special_cases() {
    assert!(matches("**/*.rs", "*"), "a star in the text is text");
    assert!(matches("ls", "ls *"), "a trailing ` *` also matches nothing");
    assert!(matches("ls -la", "ls *"));
    assert!(!matches("lsof", "ls *"));
    assert!(matches("src\\main.rs", "src/*.rs"), "backslashes count as slashes");
}
