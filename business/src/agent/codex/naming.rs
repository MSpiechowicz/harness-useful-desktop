//! The helper chat of AI naming ([plan](../../../../docs/plans/codex.md), decision 9), like the
//! other harnesses' utility launch: an ephemeral thread (not saved, not listed) that never asks
//! and cannot change anything (`approvalPolicy: never`, a read-only sandbox), on the model the
//! caller picks, else the user's configured one (no model flag).
//!
//! The configured model may be one Codex refuses (a ChatGPT login cannot use every model, Verified
//! fact 2). So when the helper's first prompt fails, it is sent once more on the model `model/list`
//! marks as Codex's default. Until that is decided the failed turn does not settle the chat (the
//! mapper holds it like a waiting follow-up), so the caller waits for the second answer.

use std::sync::Arc;

use serde_json::Value;

use super::{Chat, CodexSession, Live, Queued};
use crate::codex_rpc::StartKind;
use crate::codex_rpc::mapper::TurnEnd;
use crate::codex_rpc::wire::items::UserInput;
use crate::codex_rpc::wire::outbound::{ApprovalPolicy, SandboxMode, ThreadSettings, ThreadStart};
use crate::error::Result;
use crate::model::{AgentAction, AgentReply, PromptStatus, StreamingBehavior};

/// The helper's ephemeral thread with `settings` (see the module docs).
pub(super) fn helper_thread(settings: ThreadSettings) -> ThreadStart {
    ThreadStart {
        settings: ThreadSettings {
            approval_policy: Some(ApprovalPolicy::Never),
            sandbox: Some(SandboxMode::ReadOnly),
            ..settings
        },
        ephemeral: Some(true),
        ..ThreadStart::default()
    }
}

impl CodexSession {
    /// A helper chat's prompt: kept for a second try when another model could take it.
    pub(super) async fn helper_prompt(&self, action: AgentAction) -> Result<AgentReply> {
        let chat = &self.chat;

        if let AgentAction::Prompt { message, .. } = &action {
            let mut live = chat.lock();

            if live.naming.is_none() && fallback_model(&live).is_some() {
                live.naming = Some(message.clone());
                hold(chat, &mut live, true);
            }
        }

        let sent = self.prompt_action(action).await;

        if sent.is_err() {
            let mut live = chat.lock();
            live.naming = None;
            hold(chat, &mut live, false);
        }

        sent
    }
}

impl Chat {
    /// After a turn of a helper chat that kept its prompt: a failed one goes once more on Codex's
    /// default model, then the turn's end no longer waits.
    pub(super) fn retry_naming(self: &Arc<Self>, live: &mut Live, end: &TurnEnd, owed: &mut Vec<Value>) {
        if end.kind != StartKind::Prompt {
            return;
        }

        let Some(text) = live.naming.take() else {
            return;
        };

        let fallback = fallback_model(live).filter(|_| end.status == PromptStatus::Error);

        let Some(model) = fallback else {
            let mapped = live.mapper.follow_ups_waiting(live.has_follow_ups());
            owed.extend(self.apply(live, mapped));
            return;
        };

        tracing::debug!(%model, "the helper chat's model failed, asking Codex's default model");

        live.model = Some(model);
        live.queue.push(Queued {
            client_id: uuid::Uuid::new_v4().to_string(),
            input: vec![UserInput::text(text.clone())],
            text,
            queue: StreamingBehavior::FollowUp,
            prompt: false,
        });

        // Starts it and releases the hold.
        self.next_follow_up(live);
    }
}

/// Whether a turn's end waits (`held`) instead of settling the chat (events only, nothing is owed
/// to Codex).
fn hold(chat: &Arc<Chat>, live: &mut Live, held: bool) {
    let mapped = live.mapper.follow_ups_waiting(held || live.has_follow_ups());
    chat.apply(live, mapped);
}

/// Codex's default model, when the chat does not run on it already.
fn fallback_model(live: &Live) -> Option<String> {
    let current = live.model.as_ref().or(live.thread_model.as_ref());

    live.models
        .iter()
        .find(|model| model.is_default)
        .map(|model| model.model.clone())
        .filter(|model| Some(model) != current && !model.is_empty())
}
