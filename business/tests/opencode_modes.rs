//! Approval modes as OpenCode session permission rules (`modes`), against the recorded `build` and
//! `plan` agents (`fixtures/opencode/server/agent-list.json`): the rules are decided the way
//! OpenCode decides them, the last matching rule of the agent's rules and then the session's.

mod opencode_mapper_support;

use harness_business::model::ApprovalMode;
use harness_business::opencode_server::modes::{Rule, agent_rules, deny_all, matches, session_rules};
use opencode_mapper_support::recorded_body;
use pretty_assertions::assert_eq;

/// What OpenCode decides for `permission` on `pattern` (`permission/index.ts` `evaluate`).
fn decide(agent: &[Rule], session: &[Rule], permission: &str, pattern: &str) -> String {
    agent
        .iter()
        .chain(session)
        .rev()
        .find(|rule| matches(permission, &rule.permission) && matches(pattern, &rule.pattern))
        .map_or_else(|| "ask".to_owned(), |rule| rule.action.clone())
}

/// The decisions for `calls` (`permission pattern`) of `agent` in `mode`.
fn decisions(agent: &str, mode: Option<ApprovalMode>, calls: &[(&str, &str)]) -> Vec<String> {
    let rules = agent_rules(&recorded_body("agent-list.json"), agent);
    assert!(!rules.is_empty(), "the recorded {agent} agent has rules");
    let session = session_rules(mode, &rules).unwrap_or_default();

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
    assert_eq!(session_rules(None, &[Rule::new("*", "*", "allow")]), None);

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
fn write_asks_before_edits_commands_and_subagents() {
    let mode = Some(ApprovalMode::Write);

    assert_eq!(
        decisions("build", mode, &CALLS),
        expect([
            "ask", "ask", "ask", "allow", "ask", "ask", "allow", "allow", "allow", "ask"
        ])
    );
    assert_eq!(
        decisions("plan", mode, &CALLS),
        expect([
            "ask", "deny", "ask", "allow", "ask", "deny", "allow", "allow", "allow", "ask"
        ])
    );

    let build = agent_rules(&recorded_body("agent-list.json"), "build");
    let session = session_rules(mode, &build).expect("rules");
    assert_eq!(
        session[..7],
        [
            Rule::new("edit", "*", "ask"),
            Rule::new("bash", "*", "ask"),
            Rule::new("task", "*", "ask"),
            Rule::new("*", "*", "allow"),
            Rule::new("edit", "*", "ask"),
            Rule::new("bash", "*", "ask"),
            Rule::new("task", "*", "ask"),
        ],
        "the catch-all, then the agent's wildcard allow followed by the asks it covers"
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

    let session = session_rules(mode, &[]).expect("rules");
    assert_eq!(session, [Rule::new("*", "*", "allow")]);
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
