//! Permission prompts (`can_use_tool`) as chat dialogs and the answers to them, and the hook of the
//! Always ask mode (plan Decision 3, Verified facts 1 and 3).
//!
//! - **Dialog:** a `select` with `Allow once`, `Always allow` (left out when the CLI allows no
//!   lasting rule) and `Reject`, marked as an approval and bound to the asking tool call (not for a
//!   subagent, whose calls the chat does not show). It is titled `Allow <tool>: <subject>?` (`for
//!   a subagent` when one asks), the subject on one redacted line: a command, a file (relative to
//!   the project), a URL or query, a subagent's task, else the CLI's description. Without one the
//!   CLI's own title for the prompt is the title. Why the CLI asks, when it says, is the detail
//!   of `Allow once`.
//! - **Answers:** `Allow once` lets the call run with its input. `Reject`, or a dismissed dialog,
//!   denies it with [`DECLINED`]. `Always allow` lasts until the chat ends: the request's
//!   suggested rules go back with every destination set to `session` (only rules that add an
//!   allow or a directory, never a mode change), so Claude Code saves nothing to a settings file.
//!   An ask without such rules, and every ask in Always ask mode (whose hook would ask again
//!   despite a session rule), is kept by the chat itself ([`AllowList`](super::allow_list)).
//! - **Always ask:** `initialize` registers a `PreToolUse` hook for every tool
//!   ([`ask_all_hooks`]); its callback answers `ask`, so even calls Claude Code allows on its own
//!   reach the dialog.

use std::path::Path;

use serde_json::{Map, Value};

use super::allow_list::AllowKey;
use super::mapper::PermissionAsk;
use super::tools::{BASH, EDIT, MULTI_EDIT, NOTEBOOK_EDIT, READ, TASK, WRITE};
use super::wire::outbound::HookRegistration;
use crate::error::{BusinessError, Result};
use crate::model::{AgentEvent, UiOutcome, UiPurpose, UiRequest, UiRequestKind};
use crate::security::redact_text;

/// Lets the call run once.
pub const ALLOW_ONCE: &str = "Allow once";
/// Lets it, and the calls the answer covers, run until the chat ends.
pub const ALWAYS_ALLOW: &str = "Always allow";
/// Denies the call.
pub const REJECT: &str = "Reject";

/// What the model is told about a call the user denied.
pub const DECLINED: &str = "The user declined this tool call.";

/// The callback id of the Always ask hook.
pub const ASK_ALL_CALLBACK: &str = "harness_ask_all";

/// How long an "Always allow" answer applies.
const UNTIL_END: &str = " until this chat ends";

/// The suggested rule kinds an "Always allow" answer echoes.
const KEPT_SUGGESTIONS: [&str; 2] = ["addRules", "addDirectories"];

/// The user's answer to a permission dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Once,
    Always,
    Reject,
}

/// What an "Always allow" answer to an ask keeps.
#[derive(Debug, Clone, PartialEq)]
pub enum Always {
    /// The ask's suggested rules, for the session (sent with the answer).
    Rules(Vec<Value>),
    /// A key of the chat's own list.
    Key(AllowKey),
}

impl Always {
    /// What "Always allow" means for `ask`; `None` when the CLI allows no lasting rule.
    /// `ask_all`: the chat runs the Always ask hook.
    pub fn of(ask: &PermissionAsk, ask_all: bool) -> Option<Self> {
        if !ask.always_allowed {
            return None;
        }

        let rules = session_rules(&ask.suggestions);

        if ask_all || rules.is_empty() {
            return Some(Self::Key(AllowKey::of(ask)));
        }

        Some(Self::Rules(rules))
    }

    /// What it allows, for the dialog.
    fn describe(&self, label: &str) -> String {
        match self {
            Self::Key(key) => key.describe(label),
            Self::Rules(rules) => {
                let named: Vec<String> = rules.iter().flat_map(rule_names).collect();
                redact_text(&format!("Also allows {}{UNTIL_END}", named.join(", ")))
            }
        }
    }
}

/// The `initialize` hooks of the Always ask mode: `PreToolUse` for every tool.
pub fn ask_all_hooks() -> Vec<HookRegistration> {
    vec![HookRegistration {
        event: "PreToolUse".to_owned(),
        matcher: Some(".*".to_owned()),
        callback_ids: vec![ASK_ALL_CALLBACK.to_owned()],
    }]
}

/// The dialog of `ask`, under the CLI's request id (see the module docs).
pub fn permission_dialog(ask: &PermissionAsk, project: &Path, always: Option<&Always>) -> AgentEvent {
    let asker = if ask.from_subagent { " for a subagent" } else { "" };
    let title = match (subject(ask, project), &ask.title) {
        (Some(subject), _) => format!("Allow {}{asker}: {subject}?", ask.label),
        (None, Some(title)) if !ask.from_subagent => title.split_whitespace().collect::<Vec<_>>().join(" "),
        (None, _) => format!("Allow {}{asker}?", ask.label),
    };

    let mut options = vec![ALLOW_ONCE.to_owned()];
    let mut details = vec![ask.reason.as_deref().map(redact_text)];

    if let Some(always) = always {
        options.push(ALWAYS_ALLOW.to_owned());
        details.push(Some(always.describe(&ask.label)));
    }

    options.push(REJECT.to_owned());
    details.push(None);

    let kind = UiRequestKind::Select {
        title: redact_text(&title),
        options,
        details,
        timeout_ms: None,
    };

    AgentEvent::UiRequest {
        request: UiRequest {
            purpose: Some(UiPurpose::Approval),
            tool_call_id: Some(ask.tool_call_id.clone()).filter(|id| !id.is_empty() && !ask.from_subagent),
            ..UiRequest::new(ask.request_id.clone(), kind)
        },
    }
}

/// The user's answer in `outcome`; `always_offered` when the dialog had `Always allow`.
pub fn choice(outcome: &UiOutcome, always_offered: bool) -> Result<Choice> {
    match outcome {
        UiOutcome::Value { value } if value == ALLOW_ONCE => Ok(Choice::Once),
        UiOutcome::Value { value } if value == ALWAYS_ALLOW && always_offered => Ok(Choice::Always),
        UiOutcome::Value { value } if value == REJECT => Ok(Choice::Reject),
        UiOutcome::Value { value } => Err(BusinessError::InvalidInput(format!(
            "{value} is not one of the options"
        ))),
        UiOutcome::Confirmed { confirmed: true } => Ok(Choice::Once),
        _ => Ok(Choice::Reject),
    }
}

/// `suggestions` as rules for this session only: the ones that add an allow rule or a directory,
/// each with its destination set to `session`.
pub fn session_rules(suggestions: &[Value]) -> Vec<Value> {
    suggestions
        .iter()
        .filter_map(Value::as_object)
        .filter(|rule| {
            let kind = rule.get("type").and_then(Value::as_str).unwrap_or_default();
            let behavior = rule.get("behavior").and_then(Value::as_str);

            KEPT_SUGGESTIONS.contains(&kind) && (kind != "addRules" || behavior == Some("allow"))
        })
        .map(|rule| {
            let mut rule: Map<String, Value> = rule.clone();
            rule.insert("destination".to_owned(), Value::from("session"));
            Value::Object(rule)
        })
        .collect()
}

/// The rules or directories one suggested rule names (`Bash(git status:*)`, `access to /tmp`).
fn rule_names(rule: &Value) -> Vec<String> {
    let list = |key: &str| rule.get(key).and_then(Value::as_array).cloned().unwrap_or_default();

    if rule.get("type").and_then(Value::as_str) == Some("addDirectories") {
        return list("directories")
            .iter()
            .filter_map(Value::as_str)
            .map(|directory| format!("access to {directory}"))
            .collect();
    }

    list("rules")
        .iter()
        .filter_map(|entry| {
            let tool = entry.get("toolName").and_then(Value::as_str)?;

            Some(match entry.get("ruleContent").and_then(Value::as_str) {
                Some(content) => format!("{tool}({content})"),
                None => format!("every {tool} call"),
            })
        })
        .collect()
}

/// What `ask` is about, on one redacted line (see the module docs).
fn subject(ask: &PermissionAsk, project: &Path) -> Option<String> {
    let field = |keys: &[&str]| keys.iter().find_map(|key| text(&ask.input, key));

    let named = match ask.tool.as_str() {
        BASH => field(&["command"]),
        READ | WRITE | EDIT | MULTI_EDIT => field(&["file_path"]).map(|path| relative(&path, project)),
        NOTEBOOK_EDIT => field(&["notebook_path"]).map(|path| relative(&path, project)),
        "WebFetch" | "WebSearch" => field(&["url", "query"]),
        TASK | "Agent" => field(&["description", "subagent_type"]),
        _ => None,
    };

    let subject = named.or_else(|| ask.description.clone())?;
    let line = subject.split_whitespace().collect::<Vec<_>>().join(" ");

    (!line.is_empty()).then(|| redact_text(&line))
}

fn relative(path: &str, project: &Path) -> String {
    Path::new(path)
        .strip_prefix(project)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .ok()
        .filter(|relative| !relative.is_empty())
        .unwrap_or_else(|| path.to_owned())
}

/// A non-empty string field.
fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}
