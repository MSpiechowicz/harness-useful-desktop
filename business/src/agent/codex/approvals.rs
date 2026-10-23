//! The app's approval modes and project trust on a Codex chat (plan Decision 2).
//!
//! - **Modes:** Always ask is Codex's `untrusted` approval policy and Write its `on-request`, both
//!   in a `workspace-write` sandbox. Yolo is `never` with `danger-full-access`, offered where
//!   `capabilities.yoloMode` lets the manager pass it. Codex has no Auto: refused before anything
//!   spawns.
//! - **Where:** the mode goes with the thread's settings (`thread/start`, `thread/resume`), which
//!   name only the sandbox mode, so Codex keeps the writable roots and network access of the
//!   user's `[sandbox_workspace_write]`. A turn's `sandboxPolicy` would have to spell those out, so
//!   the mode never changes per turn: changing it restarts the chat, which resumes its thread with
//!   the new mode.
//! - **Override:** the policy and sandbox go with `approvalsReviewer: user`, so the user answers
//!   every ask. When Codex answers the thread with another policy, sandbox or reviewer (settings
//!   it must keep, such as an organisation's requirements), the chat warns and its mode follows
//!   Codex's policy, also for the `!command` gate ([`followed_mode`]).
//! - **No override:** nothing is sent and the user's `config.toml` applies. The chat's mode is
//!   then the one Codex answers the thread with (by its approval policy).
//! - **More than the mode:** with or without an override, the chat warns when Codex's answer gives
//!   the thread more than its mode shows: folders outside the project to write in, network access,
//!   and without an override a model reviewer or full access outside Yolo. A chat shows each
//!   warning once, not again for every thread it moves to (`ops.rs`).
//! - **`!command`s** (`thread/shellCommand`) run outside Codex's sandbox without asking: refused in
//!   Always ask ([`codex_bash_allowed`]), confirmed by the user in the app otherwise.
//! - **Asks** become the app's approval dialogs (`dialogs.rs`): `Always allow` is Codex's
//!   `acceptForSession`, for a command only when Codex offers it. The app keeps no allow-list of
//!   its own and allows nothing Codex asks about without the user.
//! - **Trust:** an untrusted folder whose own `.codex` layer Codex ignores gets a notice when the
//!   chat opens, unless the launch trusted it once (see [`crate::harnesses::codex::project_trust`]).

use std::path::Path;

use serde_json::Value;

use super::{Chat, CodexSession};
use crate::codex_rpc::wire::outbound::{ApprovalPolicy, SandboxMode, ThreadSettings};
use crate::codex_rpc::wire::results::ThreadSession;
use crate::error::{BusinessError, Result};
use crate::harnesses::codex::{ProjectTrust, project_trust};
use crate::model::{AgentEvent, ApprovalMode, NoticeLevel};

/// Why a `!command` is refused in Always ask.
const BASH_IN_ALWAYS_ASK: &str = "Always ask runs no !commands in a Codex chat, as Codex runs them outside its sandbox without asking. Switch to Write to run them";

/// Why an untrusted project's own settings do not apply.
const UNTRUSTED_PROJECT: &str = "Codex does not trust this folder, so it ignores the project's .codex settings, hooks and rules. Trust the project once to use them in this chat";

/// Codex's approval policy and sandbox for the app's `mode`; `None` keeps the user's
/// configuration. Auto is refused: Codex has no such mode.
pub(super) fn mode_policy(mode: Option<ApprovalMode>) -> Result<Option<(ApprovalPolicy, SandboxMode)>> {
    let policy = match mode {
        None => return Ok(None),
        Some(ApprovalMode::AlwaysAsk) => (ApprovalPolicy::Untrusted, SandboxMode::WorkspaceWrite),
        Some(ApprovalMode::Write) => (ApprovalPolicy::OnRequest, SandboxMode::WorkspaceWrite),
        Some(ApprovalMode::Yolo) => (ApprovalPolicy::Never, SandboxMode::DangerFullAccess),
        Some(ApprovalMode::Auto) => {
            return Err(BusinessError::Unsupported(
                "Codex chats have no Auto mode: choose Always ask, Write or Yolo".to_owned(),
            ));
        }
    };

    Ok(Some(policy))
}

/// Refuses a `!command` while the chat's mode is Always ask (see the module docs).
pub fn codex_bash_allowed(mode: Option<ApprovalMode>) -> Result<()> {
    match mode {
        Some(ApprovalMode::AlwaysAsk) => Err(BusinessError::Forbidden(BASH_IN_ALWAYS_ASK.to_owned())),
        _ => Ok(()),
    }
}

/// The reviewer an override asks for: the user.
const USER_REVIEWER: &str = "user";

/// The chat's mode for the thread Codex answered with `session` in `project` when the chat asked
/// for `requested` (see the module docs), and the warning when Codex runs it otherwise or gives it
/// more than the mode shows ([`widened`]).
pub(super) fn followed_mode(
    requested: Option<ApprovalMode>,
    session: &ThreadSession,
    project: &Path,
) -> (Option<ApprovalMode>, Option<AgentEvent>) {
    let answered = answered_mode(session.approval_policy.as_ref());
    let sandbox_type = session
        .sandbox
        .as_ref()
        .and_then(|sandbox| sandbox.get("type"))
        .and_then(Value::as_str);
    let reviewer = session
        .approvals_reviewer
        .as_deref()
        .filter(|reviewer| *reviewer != USER_REVIEWER);

    let Some((policy, sandbox)) = mode_policy(requested).ok().flatten() else {
        let mut parts = Vec::new();

        if let Some(reviewer) = reviewer {
            parts.push(format!("approvals reviewed by {reviewer}"));
        }

        if sandbox_type == Some(SandboxMode::DangerFullAccess.policy_type()) && answered != Some(ApprovalMode::Yolo) {
            parts.push("full access without a sandbox".to_owned());
        }

        parts.extend(widened(session, project));

        return (
            answered,
            given_more(&parts, "Codex's settings give this chat more than its mode shows"),
        );
    };

    let answered_policy = match &session.approval_policy {
        Some(Value::String(name)) => name.as_str(),
        Some(_) => "granular",
        None => policy.name(),
    };
    let answered_sandbox = sandbox_type.unwrap_or(sandbox.policy_type());
    let widening = widened(session, project);

    let differs = answered_policy != policy.name() || answered_sandbox != sandbox.policy_type() || reviewer.is_some();
    if !differs {
        let warning = given_more(&widening, "Codex's settings give this chat more than its mode");
        return (requested, warning);
    }

    let reviewed = reviewer.map(|reviewer| format!(", approvals reviewed by {reviewer}"));
    let runs = format!("{answered_policy} / {answered_sandbox}{}", reviewed.unwrap_or_default());
    let also = match widening.is_empty() {
        true => String::new(),
        false => format!(". Its settings also give it {}", widening.join(", ")),
    };
    let warning = AgentEvent::Notice {
        level: NoticeLevel::Warning,
        message: format!(
            "Codex runs this chat with {runs}, not with the mode the chat asked for: Codex's own settings keep it{also}. The chat shows what Codex runs"
        ),
    };

    // A granular policy has no mode of the app: the requested one stays (and with it the gate).
    (answered.or(requested), Some(warning))
}

/// What the sandbox of `session` allows beyond the project: folders outside it to write in (the
/// user's `writable_roots`) and network access.
fn widened(session: &ThreadSession, project: &Path) -> Vec<String> {
    let Some(sandbox) = &session.sandbox else {
        return Vec::new();
    };

    let mut parts = Vec::new();

    let roots: Vec<&str> = sandbox
        .get("writableRoots")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|root| !Path::new(root).starts_with(project))
        .collect();

    if !roots.is_empty() {
        parts.push(format!("writing also in {}", roots.join(", ")));
    }

    let network = sandbox.get("networkAccess");
    let open = network == Some(&Value::Bool(true)) || network.and_then(Value::as_str) == Some("enabled");

    if open {
        parts.push("network access".to_owned());
    }

    parts
}

/// The warning `<lead>: <parts>`; none without parts.
fn given_more(parts: &[String], lead: &str) -> Option<AgentEvent> {
    if parts.is_empty() {
        return None;
    }

    Some(AgentEvent::Notice {
        level: NoticeLevel::Warning,
        message: format!("{lead}: {}", parts.join(", ")),
    })
}

/// The app's mode for the approval policy Codex answered a thread with (a granular one has none).
pub(super) fn answered_mode(policy: Option<&Value>) -> Option<ApprovalMode> {
    match policy?.as_str()? {
        "untrusted" => Some(ApprovalMode::AlwaysAsk),
        "on-request" | "on-failure" => Some(ApprovalMode::Write),
        "never" => Some(ApprovalMode::Yolo),
        _ => None,
    }
}

/// The notice of a chat in `project` whose own layer Codex ignores; none for a helper chat or a
/// launch that trusted the project once.
pub(super) fn trust_notice(codex_home: &Path, project: &Path, trusted_once: bool, helper: bool) -> Option<AgentEvent> {
    if trusted_once || helper || project_trust(codex_home, project) != ProjectTrust::Untrusted {
        return None;
    }

    Some(AgentEvent::Notice {
        level: NoticeLevel::Info,
        message: UNTRUSTED_PROJECT.to_owned(),
    })
}

impl Chat {
    /// The settings a thread of this chat starts, resumes or forks with in `cwd`: its mode's
    /// approval policy and sandbox, nothing without an override, and the `env_vars` that pass the
    /// chat's memory identity on (`memory.rs`).
    pub(super) fn thread_settings(&self, cwd: String) -> ThreadSettings {
        let policy = mode_policy(self.approval_override).ok().flatten();
        let memory = self.lock().memory_config.clone();

        ThreadSettings {
            cwd: Some(cwd),
            approval_policy: policy.map(|(approval, _)| approval),
            sandbox: policy.map(|(_, sandbox)| sandbox),
            approvals_reviewer: policy.map(|_| USER_REVIEWER.to_owned()),
            config: (!memory.is_empty()).then_some(memory),
            ..ThreadSettings::default()
        }
    }
}

impl CodexSession {
    /// The chat's approval mode: its override, else the one of the approval policy Codex runs
    /// the thread with; `None` when that has no app mode. A `!command` checks it with
    /// [`codex_bash_allowed`].
    pub fn approval_mode(&self) -> Option<ApprovalMode> {
        self.chat.lock().mode
    }
}
