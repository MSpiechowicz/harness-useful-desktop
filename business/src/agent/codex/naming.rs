//! The helper chat of AI naming ([plan](../../../../docs/plans/codex.md), decision 9), like the
//! other harnesses' utility launch: an ephemeral thread (not saved, not listed) that cannot
//! change anything and reaches nothing outside (a read-only sandbox, web search off and every MCP
//! server Codex lists for the project switched off for the thread, `memory.rs`), on the model the caller picks, else the
//! user's configured one (no model flag). It asks about anything not known to be safe
//! (`approvalPolicy: untrusted`), and every ask is declined at once (`dialogs.rs`). A server whose
//! name a config key path cannot hold stays as configured.
//!
//! The configured model may be one Codex refuses (a ChatGPT login cannot use every model, Verified
//! fact 2). So when the helper's first prompt fails, it is sent once more on the model `model/list`
//! marks as Codex's default. Until that is decided the failed turn does not settle the chat (the
//! mapper holds it like a waiting follow-up), so the caller waits for the second answer.

use std::sync::Arc;

use serde_json::{Map, Value, json};

use super::memory::McpServers;
use super::{Chat, CodexSession, Live, Queued};
use crate::codex_rpc::StartKind;
use crate::codex_rpc::mapper::TurnEnd;
use crate::codex_rpc::wire::items::UserInput;
use crate::codex_rpc::wire::outbound::{ApprovalPolicy, SandboxMode, ThreadSettings, ThreadStart};
use crate::error::Result;
use crate::harnesses::codex::is_plain_name;
use crate::model::{AgentAction, AgentReply, PromptStatus, StreamingBehavior};

/// The helper's ephemeral thread with `settings`, switching off the MCP servers `servers` Codex
/// listed (see the module docs).
pub(super) fn helper_thread(settings: ThreadSettings, servers: &McpServers) -> ThreadStart {
    let mut config = Map::new();
    config.insert("web_search".to_owned(), json!("disabled"));

    for name in servers.keys().filter(|name| is_plain_name(name)) {
        config.insert(format!("mcp_servers.{name}.enabled"), json!(false));
    }

    ThreadStart {
        settings: ThreadSettings {
            approval_policy: Some(ApprovalPolicy::Untrusted),
            sandbox: Some(SandboxMode::ReadOnly),
            approvals_reviewer: Some("user".to_owned()),
            config: Some(config),
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
