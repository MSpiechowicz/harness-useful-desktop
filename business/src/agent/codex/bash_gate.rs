//! Whether a Codex chat may run a `!command` ([plan](../../../../docs/plans/codex.md), decision
//! 5). Codex runs it unsandboxed with full access, so the app keeps its usual confirmation
//! before every `!command` and the policy of each approval mode lives here alone. The approval
//! modes (and so which mode refuses) belong to the approval-modes step of the plan: until it
//! lands every chat runs on the user's `config.toml`, without a mode of the app, and is allowed.

use super::Chat;
use crate::error::Result;
use crate::model::ApprovalMode;

impl Chat {
    /// The approval mode the chat runs in: `None` while it runs on the user's configuration.
    pub(super) fn approval_mode(&self) -> Option<ApprovalMode> {
        None
    }
}

/// Allows or refuses a `!command` in a chat running in approval `mode` (`None`: the user's
/// configuration).
pub(super) fn bash_allowed(_mode: Option<ApprovalMode>) -> Result<()> {
    Ok(())
}
