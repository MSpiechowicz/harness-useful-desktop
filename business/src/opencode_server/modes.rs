//! Per-chat approval modes as OpenCode session permission rules. Pure: no I/O.
//!
//! OpenCode decides a tool call with the *last* matching rule of the agent's rules followed by the
//! session's (`permission/index.ts` `evaluate`, `findLast`). `PATCH /session/{id} {permission}`
//! *appends* to the session's stored rules (`Permission.merge` is a plain concatenation, no
//! dedupe) and they stay in OpenCode's database. Every list the chat sets therefore holds a
//! catch-all (`*` on `*`): the last list decides every call on its own, whatever earlier lists
//! the session holds ([`ends_with`] tells when a session already ends with a list).
//!
//! The session rules of a mode are a leading catch-all followed by the agent's own rules,
//! transformed, so the agent's denies and its order still apply:
//!
//! - **Always ask:** `*:*:ask`, then every `allow` becomes `ask`, except for `question`,
//!   `todowrite` and `plan_*` (asking before asking the user is no help). A wildcard `allow` that
//!   covers one of them is followed by an `allow` for it.
//! - **Write:** the same with a closed read-only allowlist kept: `read`, `glob`, `grep`, `list`,
//!   `lsp` (the same file queries `read` warms up), `skill` (loads a skill's instructions),
//!   `todowrite`, `question` and `plan_*`. Everything else asks: edits, `bash`, `task` (a
//!   subagent inherits only the parent's denies), `webfetch`, `websearch`, MCP, plugin and custom
//!   tools. The agent's `external_directory` rules stay as they are: every tool that changes
//!   something asks on its own, so they only let reads outside the project through.
//! - **Yolo:** `*:*:allow`, then every `ask` becomes `allow`. Denies stay, so the plan agent stays
//!   read-only, and an `ask` that reopens what an earlier agent rule denies stays an `ask` (for
//!   the plan agent and a user config with `edit: ask`, edits keep asking).
//! - **No override:** no session rules, the user's OpenCode configuration applies. A session that
//!   already holds rules (an earlier chat set a mode) gets the [`neutral`] list, which decides
//!   every call exactly as the agent's own rules do.
//!
//! A helper chat (naming a chat) denies everything ([`deny_all`]).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::ApprovalMode;

/// Permissions that never ask in "Always ask" (as OpenCode permission patterns).
const ASK_EXEMPT: [&str; 3] = ["question", "todowrite", "plan_*"];

/// Permissions that do not ask in "Write": read-only tools and the ones that never ask.
const WRITE_EXEMPT: [&str; 9] = [
    "read",
    "glob",
    "grep",
    "list",
    "lsp",
    "skill",
    "todowrite",
    "question",
    "plan_*",
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

    /// `*` on `*`: matches every call.
    fn is_catch_all(&self) -> bool {
        self.permission == "*" && self.pattern == "*"
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

/// The session rules of `mode` for an agent with `rules` (see the module docs); `None` without an
/// override.
pub fn session_rules(mode: Option<ApprovalMode>, rules: &[Rule]) -> Option<Vec<Rule>> {
    let session = match mode? {
        ApprovalMode::AlwaysAsk => gated(rules, &ASK_EXEMPT, &[]),
        ApprovalMode::Write => gated(rules, &WRITE_EXEMPT, &WRITE_KEPT),
        ApprovalMode::Yolo => relaxed(rules),
    };

    Some(session)
}

/// The session rules of a helper chat: every tool is denied.
pub fn deny_all() -> Vec<Rule> {
    vec![Rule::new("*", "*", "deny")]
}

/// Session rules that decide every call as the agent's own `rules` do: the rules themselves, which
/// hold a catch-all (OpenCode's defaults start with `*:*:allow`); without one, `*:*:ask` goes first
/// (what OpenCode decides when no rule matches).
pub fn neutral(rules: &[Rule]) -> Vec<Rule> {
    if rules.iter().any(Rule::is_catch_all) {
        return rules.to_vec();
    }

    std::iter::once(Rule::new("*", "*", "ask"))
        .chain(rules.iter().cloned())
        .collect()
}

/// True when the stored rules `stored` end with `list`: with a catch-all in `list`, the session
/// decides every call as `list` does.
pub fn ends_with(stored: &[Rule], list: &[Rule]) -> bool {
    list.iter().any(Rule::is_catch_all) && stored.ends_with(list)
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
fn gated(rules: &[Rule], exempt: &[&str], kept: &[&str]) -> Vec<Rule> {
    let mut session = vec![Rule::new("*", "*", "ask")];

    for rule in rules {
        let unchanged = !rule.is("allow")
            || exempt.iter().any(|exempt| matches(&rule.permission, exempt))
            || kept.contains(&rule.permission.as_str());

        if unchanged {
            session.push(rule.clone());
            continue;
        }

        session.push(rule.with_action("ask"));
        session.extend(covered(rule, exempt, "allow"));
    }

    session
}

/// `*:*:allow`, then the agent's `rules` with every `ask` an `allow`, unless it reopens what an
/// earlier rule denies: an `ask` inside a denied area stays, and a denied area inside an `ask`
/// keeps asking.
fn relaxed(rules: &[Rule]) -> Vec<Rule> {
    let mut session = vec![Rule::new("*", "*", "allow")];

    for (index, rule) in rules.iter().enumerate() {
        let before = &rules[..index];

        if !rule.is("ask") || denied(before, &rule.permission, &rule.pattern) {
            session.push(rule.clone());
            continue;
        }

        session.push(rule.with_action("allow"));

        let reopened = before.iter().filter(|earlier| {
            earlier.is("deny")
                && matches(&earlier.permission, &rule.permission)
                && matches(&earlier.pattern, &rule.pattern)
                && denied(before, &earlier.permission, &earlier.pattern)
        });

        session.extend(reopened.map(|area| area.with_action("ask")));
    }

    session
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
