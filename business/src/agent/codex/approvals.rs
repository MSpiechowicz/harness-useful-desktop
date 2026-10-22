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
//! - **No override:** nothing is sent and the user's `config.toml` applies. The chat's mode is
//!   then the one Codex answers the thread with (by its approval policy).
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
    /// approval policy and sandbox, nothing without an override.
    pub(super) fn thread_settings(&self, cwd: String) -> ThreadSettings {
        let policy = mode_policy(self.approval_override).ok().flatten();

        ThreadSettings {
            cwd: Some(cwd),
            approval_policy: policy.map(|(approval, _)| approval),
            sandbox: policy.map(|(_, sandbox)| sandbox),
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
