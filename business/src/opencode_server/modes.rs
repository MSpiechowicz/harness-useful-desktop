//! Per-chat approval modes as OpenCode permission rules. Pure: no I/O.
//!
//! OpenCode decides a tool call with the *last* matching rule of the agent's rules followed by the
//! session's (`permission/index.ts` `evaluate`, `findLast`). The rules of a mode for an agent
//! ([`mode_rules`]) are a leading catch-all followed by the agent's own rules, transformed, so the
//! agent's denies and its order still apply and the list decides every call on its own. The chat
//! hands them to its own server as configuration ([`mode_config`](super::mode_config)); nothing
//! is stored with the session.
//!
//! - **Always ask:** `*:*:ask`, then every `allow` becomes `ask`, except for `question`,
//!   `todowrite`, `plan_enter` and `plan_exit` (asking before asking the user is no help). A
//!   wildcard `allow` that covers one of them is followed by an `allow` for it.
//! - **Write:** the same with a closed read-only allowlist kept: `read` (also MCP resource reads),
//!   `glob`, `grep`, `list`, `lsp` (the same file queries `read` warms up), `skill` (loads a
//!   skill's instructions), `todowrite`, `question`, `plan_enter` and `plan_exit`. Everything
//!   else asks: edits, `bash`, `task`, `webfetch`, `websearch`, MCP, plugin and custom tools (a
//!   server named `plan` included). The agent's `external_directory` rules stay as they are:
//!   every tool that changes something asks on its own, so they only let reads outside the
//!   project through.
//! - **Yolo:** `*:*:allow`, then every `ask` becomes `allow`. Denies stay, so the plan agent stays
//!   read-only, and an `ask` that reopens what an earlier agent rule denies stays an `ask` (for
//!   the plan agent and a user config with `edit: ask`, edits keep asking).
//! - **No override:** no rules, the user's OpenCode configuration applies.
//!
//! A helper chat (naming a chat) denies everything ([`deny_all`]).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::ApprovalMode;

/// Permissions that never ask in "Always ask" (as OpenCode permission patterns).
const ASK_EXEMPT: [&str; 4] = ["question", "todowrite", "plan_enter", "plan_exit"];

/// Permissions that do not ask in "Write": read-only tools and the ones that never ask.
const WRITE_EXEMPT: [&str; 10] = [
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
];

/// Permissions whose agent rules "Write" keeps as they are (see the module docs).
const WRITE_KEPT: [&str; 1] = ["external_directory"];

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

    fn is(&self, action: &str) -> bool {
        self.action == action
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

/// The rules of `mode` for an agent with `rules` (see the module docs); `None` without an override.
pub fn mode_rules(mode: Option<ApprovalMode>, rules: &[Rule]) -> Option<Vec<Rule>> {
    let list = match mode? {
        ApprovalMode::AlwaysAsk => gated(rules, &ASK_EXEMPT, &[]),
        ApprovalMode::Write => gated(rules, &WRITE_EXEMPT, &WRITE_KEPT),
        ApprovalMode::Yolo => relaxed(rules),
    };

    Some(list)
}

/// The rules of a helper chat: every tool is denied.
pub fn deny_all() -> Vec<Rule> {
    vec![Rule::new("*", "*", "deny")]
}

/// The rule OpenCode applies to `permission` on `pattern`: the last of `rules` that matches;
/// `None` when none does (OpenCode then asks).
pub fn evaluate<'a>(rules: &'a [Rule], permission: &str, pattern: &str) -> Option<&'a Rule> {
    rules
        .iter()
        .rev()
        .find(|rule| matches(permission, &rule.permission) && matches(pattern, &rule.pattern))
}

/// `*:*:ask`, then the agent's `rules` with every `allow` an `ask`, except for permissions in
/// `exempt` (a wildcard `allow` that covers one is followed by an `allow` for it) and in `kept`.
/// OpenCode 2's modes use it with their own tool names.
pub(crate) fn gated(rules: &[Rule], exempt: &[&str], kept: &[&str]) -> Vec<Rule> {
    let mut list = vec![Rule::new("*", "*", "ask")];

    for rule in rules {
        let unchanged = !rule.is("allow")
            || exempt.iter().any(|exempt| matches(&rule.permission, exempt))
            || kept.contains(&rule.permission.as_str());

        if unchanged {
            list.push(rule.clone());
            continue;
        }

        list.push(rule.with_action("ask"));
        list.extend(covered(rule, exempt, "allow"));
    }

    list
}

/// `*:*:allow`, then the agent's `rules` with every `ask` an `allow`, unless it reopens what an
/// earlier rule denies: an `ask` inside a denied area stays, and a denied area inside an `ask`
/// keeps asking.
pub(crate) fn relaxed(rules: &[Rule]) -> Vec<Rule> {
    let mut list = vec![Rule::new("*", "*", "allow")];

    for (index, rule) in rules.iter().enumerate() {
        let before = &rules[..index];

        if !rule.is("ask") || denied(before, &rule.permission, &rule.pattern) {
            list.push(rule.clone());
            continue;
        }

        list.push(rule.with_action("allow"));

        let reopened = before.iter().filter(|earlier| {
            earlier.is("deny")
                && matches(&earlier.permission, &rule.permission)
                && matches(&earlier.pattern, &rule.pattern)
                && denied(before, &earlier.permission, &earlier.pattern)
        });

        list.extend(reopened.map(|area| area.with_action("ask")));
    }

    list
}

/// True when `rules` deny `permission` on `pattern` (both taken as text).
fn denied(rules: &[Rule], permission: &str, pattern: &str) -> bool {
    evaluate(rules, permission, pattern).is_some_and(|rule| rule.is("deny"))
}

/// `action` rules for each of `permissions` a wildcard `rule` covers, with its pattern.
fn covered<'a>(rule: &'a Rule, permissions: &'a [&str], action: &'a str) -> impl Iterator<Item = Rule> + 'a {
    permissions
        .iter()
        .filter(|permission| rule.permission != **permission && matches(permission, &rule.permission))
        .map(move |permission| Rule::new(permission, &rule.pattern, action))
}

/// OpenCode's wildcard match (`util/wildcard.ts`): `*` is any run of characters (a `*` in the text
/// too), `?` one character, the rest is literal, a backslash counts as `/`, and a trailing ` *` also
/// matches nothing (`ls *` matches `ls`).
pub fn matches(text: &str, pattern: &str) -> bool {
    let text: Vec<char> = text.replace('\\', "/").chars().collect();
    let pattern: Vec<char> = pattern.replace('\\', "/").chars().collect();

    if let Some(head) = pattern.strip_suffix(&[' ', '*'])
        && glob(&text, head)
    {
        return true;
    }

    glob(&text, &pattern)
}

/// A classic two-pointer glob match with backtracking to the last `*`.
fn glob(text: &[char], pattern: &[char]) -> bool {
    let (mut t, mut p) = (0, 0);
    let mut star: Option<(usize, usize)> = None;

    while t < text.len() {
        if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, t));
            p += 1;
        } else if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            t += 1;
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
