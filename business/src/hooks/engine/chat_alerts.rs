//! What [`HookSession`] hands the shell about the chat's alerts (see [`crate::hooks::alerts`]).

use super::HookSession;
use crate::model::ChatAlertKind;

impl HookSession {
    /// The alert due now, once. Call it after each event was handled and its outcome carried out,
    /// and again right after [`HookSession::on_event`] (a dialog must not wait for slow commands).
    pub fn take_alert(&mut self) -> Option<ChatAlertKind> {
        self.alerts.take()
    }

    /// The follow-up of a settled run will not go out: nothing was composed, a user message
    /// cancelled it, or sending it failed. The run's alert is due now.
    pub fn follow_up_unsent(&mut self) {
        self.alerts.release();
    }
}
