//! Hands the alerts the hook engine decided on to [`crate::chat_alerts::ChatAlerts`]: only for
//! chats that raise alerts, and only while the user did not end the chat.

use harness_business::hooks::{HookSession, exit_alert};
use harness_business::model::{AgentEvent, ChatAlertKind};

use super::{ChatHooks, ChatTask};
use crate::agents::Agents;
use crate::state::AppState;

/// Raises the alert a new, other, forked or cloned session released: a finished run waited for its
/// follow-up, which will not go out now. Called with the chat's lock held, right after the engine
/// saw the action, so the user hears of it at once and not with the chat's next event.
pub(super) fn raise_released_by_switch(state: &AppState, chat: &ChatHooks, session: &mut HookSession) {
    let Some(kind) = session.take_alert() else {
        return;
    };

    if may_raise(&state.agents, chat) {
        state.chat_alerts.raise(chat.key(), kind);
    }
}

/// A routine run's chat stays silent. So does a chat the user ended: a stop, a closed chat and
/// a page reload mark it as ended by the user (see `AgentManager::ended_by_user`) before its
/// agent exits, and the app quitting ends every chat. Not being listed any more says nothing: the
/// manager forgets a crashed agent at any look at its list, maybe before its `exited` is handled.
fn may_raise(agents: &Agents, chat: &ChatHooks) -> bool {
    chat.raises_alerts() && !agents.is_shutting_down() && !agents.manager().ended_by_user(chat.key())
}

impl ChatHooks {
    /// The chat raises alerts (a chat the user started).
    pub(super) fn raising_alerts(mut self, alerts: bool) -> Self {
        self.alerts = alerts;
        self
    }

    /// True when the chat raises alerts.
    pub(super) fn raises_alerts(&self) -> bool {
        self.alerts
    }
}

impl ChatTask {
    /// Raises what `event` made due: `due` is the engine's alert after it handled the event, and an
    /// exit the user did not ask for is a failure.
    pub(super) fn raise(&self, event: &AgentEvent, due: Option<ChatAlertKind>) {
        let due = match event {
            AgentEvent::Exited { code, signal } => exit_alert(*code, *signal),
            _ => due,
        };

        let Some(kind) = due else {
            return;
        };

        if may_raise(&self.agents, &self.chat) {
            self.chat_alerts.raise(self.chat.key(), kind);
        }
    }

    /// Raises what a follow-up released or dropped while the task carried out an outcome.
    pub(super) async fn raise_released(&self) {
        let due = self.chat.session.lock().await.take_alert();

        if let Some(kind) = due
            && may_raise(&self.agents, &self.chat)
        {
            self.chat_alerts.raise(self.chat.key(), kind);
        }
    }

    /// The chat's task is done with its exit: the manager need not remember it was ended.
    pub(super) fn forget_ended(&self) {
        self.agents.manager().forget_ended(self.chat.key());
    }
}
