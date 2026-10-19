//! Per-chat approval modes as OpenCode session permission rules. Pure: no I/O.
//!
//! OpenCode decides a tool call with the *last* matching rule of the agent's rules followed by the
//! session's (`permission/index.ts` `evaluate`, `findLast`). The session rules of a mode are
//! therefore a leading catch-all followed by the agent's own rules, transformed, so the agent's
//! denies and its order still apply:
//!
//! - **Always ask:** `*:*:ask`, then every `allow` becomes `ask`, except for `question`,
//!   `todowrite` and `plan_*` (asking before asking the user is no help). A wildcard `allow` that
//!   covers one of them is followed by an `allow` for it.
//! - **Write:** `edit`, `bash` and `task` ask (`edit` covers `write` and `apply_patch`; a `task`
//!   subagent inherits only the parent's denies, so it asks first). Their `allow` rules become
//!   `ask`; a wildcard `allow` that covers them is followed by an `ask` for each.
//! - **Yolo:** `*:*:allow`, then every `ask` becomes `allow`. Denies stay, so the plan agent stays
//!   read-only.
//! - **No override:** no session rules; the user's OpenCode configuration applies.
//!
//! A helper chat (naming a chat) denies everything ([`deny_all`]).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::ApprovalMode;

/// Permissions that never ask in "Always ask" (as OpenCode permission patterns).
const ASK_EXEMPT: [&str; 3] = ["question", "todowrite", "plan_*"];

/// Permissions that ask in "Write".
const WRITE_GATED: [&str; 3] = ["edit", "bash", "task"];

/// One OpenCode permission rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// The permission (tool) pattern, `*` for all.
    pub permission: String,
    /// The pattern of what the tool works on (command, path, …).
    pub pattern: String,
    /// `allow`, `ask` or `deny`.
    pub action: String,
}

impl Rule {
    /// A rule.
    pub fn new(permission: &str, pattern: &str, action: &str) -> Self {
        Self {
            permission: permission.to_owned(),
            pattern: pattern.to_owned(),
            action: action.to_owned(),
        }
    }

    fn with_action(&self, action: &str) -> Self {
        Self::new(&self.permission, &self.pattern, action)
    }
}

/// The rules of agent `agent` in a `GET /agent` list; empty when it is not listed.
pub fn agent_rules(agents: &Value, agent: &str) -> Vec<Rule> {
    let listed = agents.as_array().into_iter().flatten();
    let found = listed
        .filter(|candidate| candidate.get("name").and_then(Value::as_str) == Some(agent))
        .find_map(|candidate| candidate.get("permission").cloned());

    found
        .and_then(|rules| serde_json::from_value(rules).ok())
        .unwrap_or_default()
}

/// The session rules of `mode` for an agent with `rules` (see the module docs); `None` without an
/// override.
pub fn session_rules(mode: Option<ApprovalMode>, rules: &[Rule]) -> Option<Vec<Rule>> {
    let mode = mode?;
    let mut session = Vec::new();

    match mode {
        ApprovalMode::AlwaysAsk => {
            session.push(Rule::new("*", "*", "ask"));

            for rule in rules {
                if rule.action != "allow" || ASK_EXEMPT.iter().any(|exempt| matches(&rule.permission, exempt)) {
                    session.push(rule.clone());
                    continue;
                }

                session.push(rule.with_action("ask"));
                session.extend(covered(rule, &ASK_EXEMPT, "allow"));
            }
        }
        ApprovalMode::Write => {
            session.extend(WRITE_GATED.iter().map(|gated| Rule::new(gated, "*", "ask")));

            for rule in rules {
                if rule.action == "allow" && WRITE_GATED.contains(&rule.permission.as_str()) {
                    session.push(rule.with_action("ask"));
                    continue;
                }

                session.push(rule.clone());

                if rule.action == "allow" {
                    session.extend(covered(rule, &WRITE_GATED, "ask"));
                }
            }
        }
        ApprovalMode::Yolo => {
            session.push(Rule::new("*", "*", "allow"));

            let relaxed = rules.iter().map(|rule| match rule.action.as_str() {
                "ask" => rule.with_action("allow"),
                _ => rule.clone(),
            });
            session.extend(relaxed);
        }
    }

    Some(session)
}

/// The session rules of a helper chat: every tool is denied.
pub fn deny_all() -> Vec<Rule> {
    vec![Rule::new("*", "*", "deny")]
}

/// `action` rules for each of `permissions` a wildcard `rule` covers, with its pattern.
fn covered<'a>(rule: &'a Rule, permissions: &'a [&str], action: &'a str) -> impl Iterator<Item = Rule> + 'a {
    permissions
        .iter()
        .filter(|permission| rule.permission != **permission && matches(permission, &rule.permission))
        .map(move |permission| Rule::new(permission, &rule.pattern, action))
}

/// OpenCode's wildcard match (`util/wildcard.ts`): `*` is any run of characters, `?` one character,
/// the rest is literal.
pub fn matches(text: &str, pattern: &str) -> bool {
    let text: Vec<char> = text.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();

    // Classic two-pointer glob match with backtracking to the last `*`.
    let (mut t, mut p) = (0, 0);
    let mut star: Option<(usize, usize)> = None;

    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            t += 1;
            p += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, t));
            p += 1;
        } else if let Some((star_p, star_t)) = star {
            p = star_p + 1;
            t = star_t + 1;
            star = Some((star_p, star_t + 1));
        } else {
            return false;
        }
    }

    pattern[p..].iter().all(|character| *character == '*')
}
