//! Per-chat approval modes as OpenCode 2 permission rules (`{action, resource, effect}`). Pure: no
//! I/O.
//!
//! OpenCode 2 decides a tool call with the *last* matching rule of the agent's rules followed by
//! the session's (`permission.ts` `evaluate`, `findLast`; a `deny` there ends the call before
//! anything else is looked at), then the project's saved "Always allow" rules. The modes are the
//! OpenCode 1 ones ([`crate::opencode_server::modes`]), built by the same transforms with OpenCode
//! 2's action names: a leading catch-all, then the agent's own rules transformed, so its denies
//! and its order still apply and the list decides every call on its own. The chat hands them to
//! its own server as configuration ([`mode_config`](super::mode_config)); nothing is stored with
//! the session.
//!
//! - **Always ask:** `*:*:ask`, then every `allow` becomes `ask`, except for `question` and
//!   `todowrite` (asking before asking the user is no help).
//! - **Write:** the same with a closed read-only allowlist kept: `read`, `glob`, `grep`, `skill`
//!   (loads a skill's instructions), `todowrite`, `question` and the MCP resource reads
//!   (`opencode_list_mcp_resources`, `opencode_read_mcp_resource`). Everything else asks: `edit`
//!   (the edit, write and patch tools), `shell`, `subagent`, `webfetch`, `websearch`, MCP, plugin
//!   and custom tools. The agent's `external_directory` rules stay as they are.
//! - **Yolo:** `*:*:allow`, then every `ask` becomes `allow`; denies stay (the plan agent stays
//!   read-only).
//! - **No override:** no rules, the user's OpenCode configuration applies.
//!
//! A helper chat (naming a chat) denies everything ([`deny_all`]).

use serde::{Deserialize, Serialize};

use crate::model::ApprovalMode;
use crate::opencode_server::modes::{self as v1, matches};

/// Actions that never ask in "Always ask".
const ASK_EXEMPT: [&str; 2] = ["question", "todowrite"];

/// Actions that do not ask in "Write": read-only tools and the ones that never ask.
const WRITE_EXEMPT: [&str; 8] = [
    "read",
    "glob",
    "grep",
    "skill",
    "todowrite",
    "question",
    "opencode_list_mcp_resources",
    "opencode_read_mcp_resource",
];

/// Actions whose agent rules "Write" keeps as they are.
const WRITE_KEPT: [&str; 1] = ["external_directory"];

/// One OpenCode 2 permission rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rule {
    /// The action (tool permission) pattern, `*` for all.
    pub action: String,
    /// The pattern of what the tool works on (command, path, …).
    pub resource: String,
    /// `allow`, `ask` or `deny`.
    pub effect: String,
}

impl Rule {
    /// A rule.
    pub fn new(action: &str, resource: &str, effect: &str) -> Self {
        Self {
            action: action.to_owned(),
            resource: resource.to_owned(),
            effect: effect.to_owned(),
        }
    }

    /// True for a rule with `effect`.
    pub fn is(&self, effect: &str) -> bool {
        self.effect == effect
    }

    fn to_v1(&self) -> v1::Rule {
        v1::Rule::new(&self.action, &self.resource, &self.effect)
    }

    fn from_v1(rule: v1::Rule) -> Self {
        Self {
            action: rule.permission,
            resource: rule.pattern,
            effect: rule.action,
        }
    }
}

/// The rules of `mode` for an agent with `rules` (see the module docs); `None` without an override.
/// Auto is Claude Code's and refused before an OpenCode chat starts; should it reach here, it asks
/// like Always ask.
pub fn mode_rules(mode: Option<ApprovalMode>, rules: &[Rule]) -> Option<Vec<Rule>> {
    let rules: Vec<v1::Rule> = rules.iter().map(Rule::to_v1).collect();

    let list = match mode? {
        ApprovalMode::AlwaysAsk | ApprovalMode::Auto => v1::gated(&rules, &ASK_EXEMPT, &[]),
        ApprovalMode::Write => v1::gated(&rules, &WRITE_EXEMPT, &WRITE_KEPT),
        ApprovalMode::Yolo => v1::relaxed(&rules),
    };

    Some(list.into_iter().map(Rule::from_v1).collect())
}

/// The actions `mode` lets through without asking whatever the agent says; `None` when the mode
/// asks for nothing (Yolo). Auto, never an OpenCode mode, counts as Always ask.
pub fn never_asked(mode: ApprovalMode) -> Option<&'static [&'static str]> {
    match mode {
        ApprovalMode::AlwaysAsk | ApprovalMode::Auto => Some(&ASK_EXEMPT),
        ApprovalMode::Write => Some(&WRITE_EXEMPT),
        ApprovalMode::Yolo => None,
    }
}

/// The rules of a helper chat: every tool is denied.
pub fn deny_all() -> Vec<Rule> {
    vec![Rule::new("*", "*", "deny")]
}

/// The rule OpenCode 2 applies to `action` on `resource`: the last of `rules` that matches; `None`
/// when none does (OpenCode then asks).
pub fn evaluate<'a>(rules: &'a [Rule], action: &str, resource: &str) -> Option<&'a Rule> {
    rules
        .iter()
        .rev()
        .find(|rule| matches(action, &rule.action) && matches(resource, &rule.resource))
}

/// What `rules` decide for `action` on `resource`: `allow`, `ask` or `deny`.
pub fn decide<'a>(rules: &'a [Rule], action: &str, resource: &str) -> &'a str {
    evaluate(rules, action, resource).map_or("ask", |rule| rule.effect.as_str())
}
