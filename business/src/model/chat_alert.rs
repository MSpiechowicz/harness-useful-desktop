//! Why a chat tells the user something while they look elsewhere.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a chat alert is about. Raised by the app per chat, never carries message text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ChatAlertKind {
    /// The run ended for good, after any hook follow-up.
    Finished,
    /// The chat opened a dialog the user has to answer (an approval or a question).
    NeedsYou,
    /// The run ended with an error, or the agent exited unexpectedly.
    Failed,
}

/// What `chat_alerts_listen` sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum ChatAlertEvent {
    /// The app is in front and the chat is not on screen: show a toast.
    Toast {
        /// The chat's key.
        key: String,
        /// What the alert is about.
        kind: ChatAlertKind,
    },
    /// The user clicked an OS notification: open the chat. A key of no open chat is ignored.
    Open {
        /// The chat's key.
        key: String,
    },
}

/// What the page tells the app through `chat_alerts_view`, so an alert knows whether the chat is on
/// screen and what to call it. Over-long keys and titles, and entries past the 64th, are dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatAlertsView {
    /// The chat the Chat view shows right now. `None` on any other view.
    pub shown: Option<String>,
    /// Chat key to chat title, for every open chat.
    pub titles: HashMap<String, String>,
}
