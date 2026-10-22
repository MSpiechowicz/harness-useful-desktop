//! The stdout reader of a [`ClaudeSession`](super::ClaudeSession) and its stdin side.
//!
//! - **Reader:** every NDJSON frame (at most 4 MiB) is decoded and mapped by the [`ClaudeMapper`]
//!   under the chat's lock. Events go to the chat in order; hints update the chat's state there
//!   too (session id, model, commands, queue, open asks). Answers the reader owes the CLI are
//!   written once the lock is released. At the end of stdout every waiting control request fails.
//! - **Control requests** ([`Chat::control`]): the app's request under a fresh `req_<n>` id,
//!   registered in the session's pending table before it is written; the CLI's `control_response`
//!   resolves it, and a request without an answer within its limit fails as timed out.
//! - **Requests of the CLI:** permission prompts, questions, dialogs and hook callbacks go to the
//!   chat's dialogs (`dialogs.rs`), an unknown subtype is answered with an error. A prompt or
//!   question past [`MAX_OPEN_ASKS`] open ones is denied at once.
//!
//! [`ClaudeMapper`]: crate::claude_stream::ClaudeMapper

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::dialogs::Outcome;
use super::reports::reply_text;
use super::{Chat, Live};
use crate::agent::io::WriterMsg;
use crate::agent::process::Stdout;
use crate::claude_stream::mapper::{Ask, Hint};
use crate::claude_stream::wire::outbound::{
    HostRequest, control_error, control_request, next_request_id, permission_deny,
};
use crate::claude_stream::wire::{ControlResponse, decode_value};
use crate::claude_stream::{Mapped, SendKind};
use crate::error::{BusinessError, Result};
use crate::model::{AgentEvent, MessageRole, StreamingBehavior};
use crate::pi_family::rpc::{Frame, FrameReader, RequestKind, RpcResponse, await_response, encode_frame};
use crate::security::redact_text;

/// The most asks open at once; the CLI's next one is denied.
pub(super) const MAX_OPEN_ASKS: usize = 256;

/// The deny of an ask past [`MAX_OPEN_ASKS`].
const TOO_MANY_ASKS: &str = "Too many requests are waiting for the user. Answer those first.";

/// Waits for the chat (built once the process runs), then reads its stdout until it ends.
pub(super) async fn read_stdout(stdout: Stdout, chat: oneshot::Receiver<Arc<Chat>>) {
    let Ok(chat) = chat.await else {
        return;
    };

    let mut frames = FrameReader::new(stdout);

    while let Some(frame) = frames.next_frame().await {
        let answers = chat.on_frame(frame);

        for answer in answers {
            if chat.write(&answer).await.is_err() {
                break;
            }
        }
    }

    chat.shared.pending.close("the agent closed its output");
}

impl Chat {
    /// Maps `frame` and applies what it means; the answers the CLI is owed, in order.
    fn on_frame(&self, frame: Frame) -> Vec<Value> {
        let value = match frame {
            Frame::Json(value) => value,
            Frame::Fault(fault) => {
                self.shared.emit(AgentEvent::FrameError {
                    original_type: fault.original_type,
                    error: fault.error,
                });
                return Vec::new();
            }
        };

        let mut live = self.lock();
        let mapped = match decode_value(value) {
            Ok(inbound) => live.mapper.on_frame(&inbound),
            Err(error) => vec![Mapped::Event(Box::new(error.event()))],
        };

        self.apply(&mut live, mapped)
    }

    /// Emits the events of `mapped` and acts on its hints, in order; the answers the CLI is owed.
    pub(super) fn apply(&self, live: &mut Live, mapped: Vec<Mapped>) -> Vec<Value> {
        let mut answers = Vec::new();

        for item in mapped {
            match item {
                Mapped::Event(event) => {
                    note(live, &event);
                    self.shared.emit(*event);
                }
                Mapped::Hint(hint) => self.hint(live, hint, &mut answers),
            }
        }

        answers
    }

    fn hint(&self, live: &mut Live, hint: Hint, answers: &mut Vec<Value>) {
        match hint {
            Hint::SessionId(id) => {
                if live.session_id.as_deref() != Some(id.as_str()) {
                    live.session_id = Some(id);
                    self.emit_state(live);
                }
            }
            Hint::Init(init) => self.on_init(live, &init),
            Hint::PromptQueued(uuid) => self.taken(live, &uuid),
            Hint::PromptStarted(uuid) => {
                self.taken(live, &uuid);
                self.dequeue(live, std::slice::from_ref(&uuid));
            }
            Hint::PromptsTaken(uuids) => {
                for uuid in &uuids {
                    self.taken(live, uuid);
                }
                self.dequeue(live, &uuids);
            }
            Hint::PromptCompleted(uuid) | Hint::PromptCancelled(uuid) => {
                self.dequeue(live, std::slice::from_ref(&uuid));
            }
            Hint::TurnEnded(end) => {
                let model = live.running_model.clone();
                live.stats.on_result(&end.result, model.as_deref());
                self.dequeue(live, &end.prompts);

                // No settle follows a run without a model turn (a compaction, a `!command`'s
                // output): the chat learns its new state here.
                if end.quiet {
                    self.emit_state(live);
                }
            }
            Hint::Compacted(metadata) => live.stats.on_compacted(&metadata),
            Hint::Title(title) => {
                live.title = Some(title);
                self.emit_state(live);
            }
            Hint::Commands(commands) => {
                live.commands = commands;
                self.emit_commands(live);
            }
            Hint::PermissionMode(mode) => live.permission_mode = Some(mode),
            Hint::Ask(ask) if live.dialogs.len() >= MAX_OPEN_ASKS && opens_dialog(&ask) => {
                answers.push(permission_deny(ask.request_id(), TOO_MANY_ASKS));
            }
            Hint::Ask(ask) => {
                let mut out = Outcome::default();
                live.dialogs.asked(ask, &mut out);

                out.events.into_iter().for_each(|event| self.shared.emit(event));
                answers.extend(out.answers);
            }
            Hint::Withdrawn(request_id) => {
                if let Some(closed) = live.dialogs.withdrawn(&request_id) {
                    self.shared.emit(closed);
                }
            }
            Hint::HookCallback { request_id, callback } => {
                answers.push(live.dialogs.hook_answer(&request_id, &callback))
            }
            Hint::Unsupported { request_id, subtype } => {
                let error = format!("the desktop app does not handle {subtype} requests");
                answers.push(control_error(&request_id, &error));
            }
            Hint::Reply(response) => {
                self.shared.pending.resolve(rpc_response(response));
            }
            Hint::RateLimit(_) | Hint::SessionState(_) | Hint::ConversationReset(_) => {}
        }
    }

    /// Runs the prompt-taken call when the CLI takes one of the chat's prompts.
    fn taken(&self, live: &mut Live, uuid: &str) {
        let before = live.awaiting_take.len();
        live.awaiting_take.retain(|waiting| waiting != uuid);

        if live.awaiting_take.len() < before {
            self.shared.prompt_taken_now();
        }
    }

    /// Takes `uuids` out of the queue (the CLI took, finished or dropped them).
    pub(super) fn dequeue(&self, live: &mut Live, uuids: &[String]) {
        let before = live.queue.len();
        live.queue.retain(|queued| !uuids.contains(&queued.uuid));

        if live.queue.len() < before {
            self.shared.emit(live.queue_update());
        }
    }

    /// Sends `request` and waits at most `limit` for its successful reply's payload.
    pub(super) async fn control(&self, request: &HostRequest, limit: Duration) -> Result<Value> {
        let id = next_request_id();
        let frame = control_request(&id, request);
        let label = frame
            .pointer("/request/subtype")
            .and_then(Value::as_str)
            .unwrap_or("control request")
            .to_owned();
        let receiver = self.shared.pending.register(&id, RequestKind::Other)?;

        if let Err(error) = self.write(&frame).await {
            self.shared.pending.forget(&id);
            return Err(error);
        }

        let response = await_response(receiver, limit, &label, || false).await;

        if response.is_err() {
            self.shared.pending.forget(&id);
        }

        let response = response?;

        if !response.success {
            let error = redact_text(response.error.as_deref().unwrap_or("no reason given"));
            return Err(BusinessError::Protocol(format!("Claude Code refused {label}: {error}")));
        }

        Ok(response.data.unwrap_or_else(|| json!({})))
    }

    /// Writes one frame to the CLI's stdin.
    pub(super) async fn write(&self, frame: &Value) -> Result<()> {
        self.shared
            .writer
            .send(WriterMsg::Frame(encode_frame(frame)))
            .await
            .map_err(|_| BusinessError::Process("the agent no longer reads input".to_owned()))
    }

    /// Takes back message `uuid` the CLI will never see (its write failed).
    pub(super) fn unsent(&self, uuid: &str) {
        let mut live = self.lock();
        live.awaiting_take.retain(|waiting| waiting != uuid);
        self.dequeue(&mut live, &[uuid.to_owned()]);

        let mapped = live.mapper.withdrawn(uuid);
        self.apply(&mut live, mapped);
    }

    pub(super) fn emit_state(&self, live: &Live) {
        self.shared.emit(AgentEvent::State {
            state: self.state(live),
        });
    }
}

/// True for an ask that waits as a dialog (the others are answered at once).
fn opens_dialog(ask: &Ask) -> bool {
    matches!(ask, Ask::Permission(_) | Ask::Question(_))
}

/// What a sent user message is to the mapper.
pub(super) fn send_kind(queue: Option<StreamingBehavior>) -> SendKind {
    match queue {
        None => SendKind::Prompt,
        Some(StreamingBehavior::Steer) => SendKind::Steer,
        Some(StreamingBehavior::FollowUp) => SendKind::FollowUp,
    }
}

/// Counts the chat's messages and keeps the text of a finished reply for `getLastAssistantText`.
fn note(live: &mut Live, event: &AgentEvent) {
    let counts = &mut live.counts;

    match event {
        AgentEvent::ToolStart { .. } => counts.tool_calls += 1,
        AgentEvent::ToolEnd { .. } => counts.total += 1,
        AgentEvent::MessageEnd { message, .. } if message.role == MessageRole::Assistant => {
            counts.assistant += 1;
            counts.total += 1;

            if let Some(text) = reply_text(&message.blocks) {
                live.last_reply = Some(text);
            }
        }
        _ => {}
    }
}

/// A control reply as the pending table takes it.
fn rpc_response(response: ControlResponse) -> RpcResponse {
    let (success, data, error) = match response.outcome {
        Ok(payload) => (true, Some(payload), None),
        Err(error) => (false, None, Some(error)),
    };

    RpcResponse {
        id: Some(response.request_id),
        command: "control".to_owned(),
        success,
        data,
        error,
    }
}
