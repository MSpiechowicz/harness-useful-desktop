//! The stdout reader of a [`CodexSession`](super::CodexSession) and its stdin side.
//!
//! - **Reader:** every JSON-RPC frame (at most 4 MiB a line) is decoded and mapped by the
//!   [`CodexMapper`](crate::codex_rpc::CodexMapper) under the chat's lock. Events go to the chat in
//!   order; hints update the chat's state there too (title, tokens, turns, queue, open asks).
//!   Frames the reader owes Codex (answers, a delayed interrupt) are written once the lock is
//!   released. At the end of stdout every waiting request fails.
//! - **Requests** ([`Chat::call`]): the app's request under a fresh id, registered in the
//!   session's pending table before it is written; Codex's response resolves it, and a request
//!   without an answer within its limit fails as timed out. An answered `turn/start` names the
//!   turn (for steering and abort) and runs the prompt-taken call of a prompt action, in event
//!   order.
//! - **Requests of Codex:** approvals and questions go to the chat's dialogs (`dialogs.rs`);
//!   dynamic tool calls, token refreshes and unknown methods are refused at once.
//! - **Turn ends** also finish a `!command` and may send a helper chat's prompt again
//!   (`runs.rs`, `naming.rs`).

use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::{Chat, Live, dialogs::Outcome};
use crate::agent::io::WriterMsg;
use crate::agent::process::Stdout;
use crate::codex_rpc::mapper::{Hint, Mapped};
use crate::codex_rpc::wire::answers::refusal;
use crate::codex_rpc::wire::outbound::{Request, next_request_id};
use crate::codex_rpc::wire::results::TurnStarted;
use crate::codex_rpc::wire::{RequestId, Response, RpcError, decode_value};
use crate::error::{BusinessError, Result};
use crate::model::{AgentEvent, ChatBlock, ChatMessage, MessageRole, PromptStatus, StreamingBehavior};
use crate::pi_family::rpc::{Frame, FrameReader, RequestKind, RpcResponse, await_response, encode_frame};
use crate::security::redact_text;

/// What Codex answered a request with: its result, or its error.
pub(super) type Answer = std::result::Result<Value, RpcError>;

/// Waits for the chat (built once the process runs), then reads its stdout until it ends.
pub(super) async fn read_stdout(stdout: Stdout, chat: oneshot::Receiver<Arc<Chat>>) {
    let mut frames = FrameReader::new(stdout);
    let Ok(chat) = chat.await else {
        return;
    };

    while let Some(frame) = frames.next_frame().await {
        let owed = {
            let mut live = chat.lock();
            let mapped = map_frame(&mut live, frame);
            chat.apply(&mut live, mapped)
        };

        // A frame Codex no longer reads is no reason to stop reading what it writes.
        for frame in &owed {
            chat.write(frame).await.ok();
        }
    }

    // A `!command` that waits for its end learns it never comes.
    chat.lock().shell = None;
    chat.shared.pending.close("the agent closed its output");
}

/// What `frame` means for the chat; a frame that cannot be read is a `frameError`.
fn map_frame(live: &mut Live, frame: Frame) -> Vec<Mapped> {
    let event = match frame {
        Frame::Json(value) => match decode_value(value) {
            Ok(inbound) => return live.mapper.on_frame(&inbound),
            Err(error) => error.event(),
        },
        Frame::Fault(fault) => AgentEvent::FrameError {
            original_type: fault.original_type,
            error: fault.error,
        },
    };

    vec![Mapped::Event(Box::new(event))]
}

impl Chat {
    /// Emits the events of `mapped` and acts on its hints, in order; the frames Codex is owed.
    pub(super) fn apply(self: &Arc<Self>, live: &mut Live, mapped: Vec<Mapped>) -> Vec<Value> {
        let mut owed = Vec::new();

        for item in mapped {
            let event = match item {
                Mapped::Hint(hint) => {
                    self.hint(live, hint, &mut owed);
                    continue;
                }
                Mapped::Event(event) => *event,
            };

            note(live, &event);
            self.shared.emit(event);
        }

        owed
    }

    fn hint(self: &Arc<Self>, live: &mut Live, hint: Hint, owed: &mut Vec<Value>) {
        match hint {
            Hint::Response(response) => self.answered(live, response, owed),
            Hint::Title(title) => {
                live.title = Some(title);
                self.emit_state(live);
            }
            Hint::TokenUsage(usage) => live.stats.on_token_usage(&usage),
            Hint::TurnStarted(id) => {
                if !id.is_empty() {
                    live.turn_id = Some(id);
                }

                self.interrupt_if_pending(live, owed);
            }
            Hint::TurnEnded(end) => {
                live.turn_id = None;
                live.abort_pending = false;
                self.drop_steering(live);

                if end.status != PromptStatus::Aborted {
                    self.next_follow_up(live);
                }

                self.turn_ended(live, &end, owed);
            }
            Hint::UserMessage(message) => {
                let position = live.queue.iter().position(|queued| {
                    queued.queue == StreamingBehavior::Steer
                        && match &message.client_id {
                            Some(id) => *id == queued.client_id,
                            None => queued.text == message.text,
                        }
                });

                if let Some(position) = position {
                    live.queue.remove(position);
                    self.shared.emit(live.queue_update());
                }
            }
            Hint::Ask(ask) => {
                let mut out = Outcome::default();
                live.dialogs.asked(ask, &mut out);

                out.events.into_iter().for_each(|event| self.shared.emit(event));
                owed.extend(out.answers);
            }
            Hint::Resolved(request_id) => {
                if let Some(closed) = live.dialogs.withdrawn(&request_id) {
                    self.shared.emit(closed);
                }
            }
            Hint::Unsupported(request) => owed.push(refusal(&request)),
            Hint::Deprecation(summary) => tracing::debug!(%summary, "Codex reports a deprecated API"),
            Hint::Shell(run) => self.shell_output(live, run),
            Hint::ThreadStarted(_) | Hint::ThreadStatus(_) => {}
        }
    }

    /// Codex answered one of the app's requests. An answered `turn/start` names the running turn
    /// and runs the prompt-taken call of a prompt action, in event order.
    fn answered(&self, live: &mut Live, response: Response, owed: &mut Vec<Value>) {
        let id = response.id.to_string();
        let position = live.turn_requests.iter().position(|(request, _)| *request == id);

        if let Some(position) = position {
            let (_, prompt) = live.turn_requests.remove(position);

            if let Ok(started) = response.result::<TurnStarted>() {
                if live.mapper.is_working() && !started.turn.id.is_empty() {
                    live.turn_id = Some(started.turn.id);
                }

                if prompt {
                    self.shared.prompt_taken_now();
                }

                self.interrupt_if_pending(live, owed);
            }
        }

        self.shared.pending.resolve(rpc_response(response));
    }

    /// Interrupts the running turn when the user stopped it before its id was known. Its answer
    /// is not awaited (the reader cannot wait).
    fn interrupt_if_pending(&self, live: &mut Live, owed: &mut Vec<Value>) {
        if !live.abort_pending {
            return;
        }

        let (Some(thread_id), Some(turn_id)) = (live.thread_id.clone(), live.running_turn()) else {
            return;
        };

        live.abort_pending = false;
        owed.push(Request::TurnInterrupt { thread_id, turn_id }.frame(&next_request_id()));
    }

    /// Steering a turn did not take ends with it: its queue entries go.
    fn drop_steering(&self, live: &mut Live) {
        let before = live.queue.len();
        live.queue.retain(|queued| queued.queue != StreamingBehavior::Steer);

        if live.queue.len() < before {
            self.shared.emit(live.queue_update());
        }
    }

    /// Sends `request` and waits at most `limit` for Codex's answer. `Err` when it was not
    /// answered (written, timed out, the process ended).
    pub(super) async fn call(&self, request: &Request, limit: Duration) -> Result<Answer> {
        let id = next_request_id();
        let (key, receiver) = self.register(&id)?;

        self.finish_call(&key, &request.frame(&id), request.method(), receiver, limit)
            .await
    }

    /// Registers request `id` in the pending table: its key and the receiver of its answer.
    pub(super) fn register(&self, id: &RequestId) -> Result<(String, oneshot::Receiver<Result<RpcResponse>>)> {
        let key = id.to_string();
        let receiver = self.shared.pending.register(&key, RequestKind::Other)?;

        Ok((key, receiver))
    }

    /// Writes the registered request `frame` (`key`) and waits at most `limit` for its answer.
    pub(super) async fn finish_call(
        &self,
        key: &str,
        frame: &Value,
        method: &str,
        receiver: oneshot::Receiver<Result<RpcResponse>>,
        limit: Duration,
    ) -> Result<Answer> {
        if let Err(error) = self.write(frame).await {
            self.shared.pending.forget(key);
            return Err(error);
        }

        let response = await_response(receiver, limit, method, || false).await;

        if response.is_err() {
            self.shared.pending.forget(key);
        }

        Ok(answer(response?))
    }

    /// Sends `request` and reads its result as `T`; Codex's error fails with its message.
    pub(super) async fn request<T: DeserializeOwned>(&self, request: &Request, limit: Duration) -> Result<T> {
        let value = self
            .call(request, limit)
            .await?
            .map_err(|error| refused(request.method(), &error))?;

        serde_json::from_value(value)
            .map_err(|_| BusinessError::Protocol(format!("Codex sent an unreadable answer to {}", request.method())))
    }

    /// Writes one frame to Codex's stdin.
    pub(super) async fn write(&self, frame: &Value) -> Result<()> {
        let bytes = encode_frame(frame);

        match self.shared.writer.send(WriterMsg::Frame(bytes)).await {
            Ok(()) => Ok(()),
            Err(_) => Err(BusinessError::Process("the agent no longer reads input".to_owned())),
        }
    }

    pub(super) fn emit_state(&self, live: &Live) {
        self.shared.emit(AgentEvent::State {
            state: self.state(live),
        });
    }
}

/// Why Codex refused a request of `method`, redacted.
pub(super) fn refused(method: &str, error: &RpcError) -> BusinessError {
    let message = redact_text(error.message.trim());
    let reason = if message.is_empty() {
        "no reason given"
    } else {
        &message
    };

    BusinessError::Protocol(format!("Codex refused {method}: {reason}"))
}

/// Counts the chat's messages and keeps the text of a finished reply for `getLastAssistantText`.
fn note(live: &mut Live, event: &AgentEvent) {
    match event {
        AgentEvent::ToolStart { .. } => live.counts.tool_calls += 1,
        AgentEvent::ToolEnd { .. } => live.counts.total += 1,
        AgentEvent::MessageEnd { message, .. } => noted_reply(live, message),
        _ => {}
    }
}

fn noted_reply(live: &mut Live, message: &ChatMessage) {
    if message.role != MessageRole::Assistant {
        return;
    }

    live.counts.assistant += 1;
    live.counts.total += 1;

    let mut text = String::new();

    for block in &message.blocks {
        if let ChatBlock::Text { text: part } = block
            && !part.is_empty()
        {
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            text.push_str(part);
        }
    }

    if !text.is_empty() {
        live.last_reply = Some(text);
    }
}

/// A response as the pending table takes it; an error keeps its code in `data`.
fn rpc_response(response: Response) -> RpcResponse {
    let (success, data, error) = match response.outcome {
        Ok(result) => (true, Some(result), None),
        Err(error) => {
            let data = json!({ "code": error.code, "message": error.message });
            (false, Some(data), Some(error.message))
        }
    };

    RpcResponse {
        id: Some(response.id.to_string()),
        command: "codex".to_owned(),
        success,
        data,
        error,
    }
}

/// What the pending table resolved, as Codex's answer.
fn answer(response: RpcResponse) -> Answer {
    let data = response.data.unwrap_or(Value::Null);

    if response.success {
        return Ok(data);
    }

    Err(RpcError {
        code: data.get("code").and_then(Value::as_i64).unwrap_or_default(),
        message: response.error.unwrap_or_default(),
        data: None,
    })
}
