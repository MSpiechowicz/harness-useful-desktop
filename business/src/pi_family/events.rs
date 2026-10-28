//! Decoding of the session-event stream shared by the PI-family harnesses.
//!
//! OMP and PI emit the same `AgentSessionEvent` shapes with a few renames (OMP
//! `auto_compaction_*`, PI `compaction_*`; OMP `thinkingLevel`, PI `level`), so one decoder handles
//! the union. Dialects intercept what differs (PI `agent_settled` also completes prompts).
//! Event types known to carry nothing the desktop shows are dropped; any other unknown type
//! becomes [`AgentEvent::Unknown`].

use std::path::Path;

use serde_json::Value;

use super::normalize::{chat_message, model_option, string, strings, tool_result, unsigned};
use super::rpc::{DecodeState, Inbound, RpcResponse};
use super::touched::touched_paths;
use super::ui_request::{decode_ui_request, notice_level};
use crate::model::{AgentEvent, MessageRole, PromptStatus};
use crate::security::redact_text;

/// Event types that are understood but not shown (subagents, live voice, host callbacks, cache
/// warming, todos, goals, bash streaming, …).
const IGNORED: &[&str] = &[
    "tool_stream_update",
    "model_changed",
    "config_warnings_changed",
    "advisor_cost_changed",
    "advisor_yielded",
    "cache_warming_start",
    "cache_warming_end",
    "retry_fallback_applied",
    "retry_fallback_succeeded",
    "ttsr_triggered",
    "todo_reminder",
    "todo_auto_clear",
    "irc_message",
    "goal_updated",
    "subagent_lifecycle",
    "subagent_progress",
    "subagent_event",
    "live_phase",
    "live_levels",
    "live_transcript",
    "live_end",
    "host_tool_call",
    "host_tool_cancel",
    "host_uri_request",
    "host_uri_cancel",
    "bash_execution_update",
    "entry_appended",
    "summarization_retry_scheduled",
    "summarization_retry_attempt_start",
    "summarization_retry_finished",
    "extension_ui_response",
];

/// The frame's `type`.
pub fn frame_type(frame: &Value) -> &str {
    frame.get("type").and_then(Value::as_str).unwrap_or_default()
}

/// A `response` frame.
pub fn response(frame: &Value) -> RpcResponse {
    RpcResponse {
        id: string(frame, "id"),
        command: string(frame, "command").unwrap_or_default(),
        success: frame.get("success").and_then(Value::as_bool).unwrap_or(false),
        data: frame.get("data").cloned(),
        error: string(frame, "error"),
        code: string(frame, "code"),
    }
}

/// Decodes a frame every PI-family harness can send.
pub fn decode_common(frame: &Value, state: &mut DecodeState) -> Vec<Inbound> {
    let kind = frame_type(frame);
    let one = |event: AgentEvent| vec![Inbound::Event(event)];

    match kind {
        "response" => vec![Inbound::Response(response(frame))],
        "extension_ui_request" => decode_ui_request(frame),
        "available_commands_update" => vec![Inbound::Commands(commands(frame))],
        "agent_start" => one(AgentEvent::AgentStart),
        "agent_end" => one(AgentEvent::AgentEnd),
        "turn_start" => one(AgentEvent::TurnStart),
        "turn_end" => one(AgentEvent::TurnEnd),
        "message_start" | "message_update" | "message_end" => message_event(frame, kind, state),
        "tool_execution_start" | "tool_execution_update" | "tool_execution_end" => tool_event(frame, kind, state),
        "prompt_result" => one(prompt_result(frame)),
        "session_settled" => one(AgentEvent::Settled { aborted: None }),
        "agent_settled" => one(AgentEvent::Settled {
            aborted: frame.get("aborted").and_then(Value::as_bool),
        }),
        "queue_update" => one(AgentEvent::QueueUpdate {
            steering: strings(frame, "steering"),
            follow_up: strings(frame, "followUp"),
            live_steered: frame
                .get("liveSteered")
                .and_then(Value::as_u64)
                .and_then(|count| u32::try_from(count).ok())
                .unwrap_or(0),
        }),
        "compaction_start" | "auto_compaction_start" => one(AgentEvent::Compaction {
            phase: "start".to_owned(),
            reason: string(frame, "reason"),
            error: None,
        }),
        "compaction_end" | "auto_compaction_end" => one(AgentEvent::Compaction {
            phase: "end".to_owned(),
            reason: string(frame, "reason"),
            error: string(frame, "errorMessage").map(|error| redact_text(&error)),
        }),
        "auto_retry_start" | "auto_retry_end" => one(retry(frame, kind)),
        _ => decode_session_info(frame, kind),
    }
}

/// Model, title, notice and diagnostic frames.
fn decode_session_info(frame: &Value, kind: &str) -> Vec<Inbound> {
    let mut events = Vec::new();

    match kind {
        "thinking_level_changed" => {
            if let Some(level) = string(frame, "thinkingLevel").or_else(|| string(frame, "level")) {
                events.push(AgentEvent::ThinkingChanged { level });
            }
        }
        "config_update" => {
            if let Some(model) = frame.get("model").and_then(model_option) {
                events.push(AgentEvent::ModelChanged { model: Some(model) });
            }

            if let Some(level) = string(frame, "thinkingLevel") {
                events.push(AgentEvent::ThinkingChanged { level });
            }
        }
        "session_info_update" | "session_info_changed" => {
            if let Some(title) = string(frame, "title").or_else(|| string(frame, "name")) {
                events.push(AgentEvent::Title { title });
            }
        }
        "notice" => events.push(AgentEvent::Notice {
            level: notice_level(frame.get("level")),
            message: redact_text(&string(frame, "message").unwrap_or_default()),
        }),
        "extension_error" => events.push(extension_error(frame)),
        "command_output" => events.push(AgentEvent::CommandOutput {
            text: string(frame, "text").unwrap_or_default(),
        }),
        "rpc_frame_error" => events.push(AgentEvent::FrameError {
            original_type: string(frame, "originalType"),
            error: string(frame, "error").unwrap_or_else(|| "frame dropped by the agent".to_owned()),
        }),
        _ if IGNORED.contains(&kind) => {}
        _ => events.push(AgentEvent::Unknown {
            event_type: kind.chars().take(64).collect(),
        }),
    }

    events.into_iter().map(Inbound::Event).collect()
}

fn commands(frame: &Value) -> Vec<Value> {
    frame
        .get("commands")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn message_event(frame: &Value, kind: &str, state: &mut DecodeState) -> Vec<Inbound> {
    let given = frame.get("messageId").and_then(Value::as_str);
    let message = frame.get("message").cloned().unwrap_or(Value::Null);

    // Kept out of the harness's own chat view; both its start and end frames carry the message.
    if super::normalize::hidden_message(&message) {
        return Vec::new();
    }

    let event = match kind {
        "message_start" => AgentEvent::MessageStart {
            message_id: state.open_message(given),
            role: super::normalize::message_role(&message),
        },
        "message_update" => match delta(frame, state.current_message(given)) {
            Some(event) => event,
            None => return Vec::new(),
        },
        _ => {
            let message_id = state.close_message(given);
            let message = chat_message(&message);

            if message.role == MessageRole::Assistant {
                state.note_assistant_end(&message);
            }

            AgentEvent::MessageEnd { message_id, message }
        }
    };

    vec![Inbound::Event(event)]
}

/// A text or thinking delta of `message_update`; other stream updates are dropped (`message_end`
/// carries the final content).
fn delta(frame: &Value, message_id: String) -> Option<AgentEvent> {
    let update = frame.get("assistantMessageEvent")?;
    let content_index = u32::try_from(unsigned(update, "contentIndex")?).ok()?;
    let delta = string(update, "delta")?;

    match update.get("type").and_then(Value::as_str)? {
        "text_delta" => Some(AgentEvent::TextDelta {
            message_id,
            content_index,
            delta,
        }),
        "thinking_delta" => Some(AgentEvent::ThinkingDelta {
            message_id,
            content_index,
            delta,
        }),
        _ => None,
    }
}

fn tool_event(frame: &Value, kind: &str, state: &mut DecodeState) -> Vec<Inbound> {
    let Some(tool_call_id) = string(frame, "toolCallId") else {
        return Vec::new();
    };
    let name = string(frame, "toolName").unwrap_or_default();

    let event = match kind {
        "tool_execution_start" => {
            let args = frame.get("args").cloned().unwrap_or(Value::Null);
            state.remember_tool(&tool_call_id, &name, &args);

            AgentEvent::ToolStart {
                tool_call_id,
                name,
                args,
                intent: string(frame, "intent"),
            }
        }
        "tool_execution_update" => AgentEvent::ToolUpdate {
            tool_call_id,
            partial: tool_result(frame.get("partialResult")),
        },
        _ => {
            let args = state
                .take_tool_args(&tool_call_id)
                .or_else(|| frame.get("args").cloned())
                .unwrap_or(Value::Null);

            AgentEvent::ToolEnd {
                touched_paths: touched_paths(state.project(), &name, &args),
                tool_call_id,
                name,
                is_error: frame.get("isError").and_then(Value::as_bool).unwrap_or(false),
                result: tool_result(frame.get("result")),
            }
        }
    };

    vec![Inbound::Event(event)]
}

/// OMP `prompt_result`.
fn prompt_result(frame: &Value) -> AgentEvent {
    let status = match frame.get("status").and_then(Value::as_str) {
        Some("aborted") => PromptStatus::Aborted,
        Some("error") => PromptStatus::Error,
        _ => PromptStatus::Completed,
    };
    let error = frame.get("error");

    AgentEvent::PromptResult {
        request_id: string(frame, "id"),
        status,
        error: error
            .and_then(|error| string(error, "message"))
            .map(|message| redact_text(&message)),
        retryable: error
            .and_then(|error| error.get("retryable"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn retry(frame: &Value, kind: &str) -> AgentEvent {
    let attempt = unsigned(frame, "attempt").and_then(|number| u32::try_from(number).ok());

    if kind == "auto_retry_start" {
        return AgentEvent::Retry {
            phase: "start".to_owned(),
            attempt,
            max_attempts: unsigned(frame, "maxAttempts").and_then(|number| u32::try_from(number).ok()),
            message: string(frame, "errorMessage").map(|message| redact_text(&message)),
        };
    }

    AgentEvent::Retry {
        phase: "end".to_owned(),
        attempt,
        max_attempts: None,
        message: string(frame, "finalError").map(|message| redact_text(&message)),
    }
}

/// An extension handler threw; only the extension's file name and the redacted error are shown.
fn extension_error(frame: &Value) -> AgentEvent {
    let path = string(frame, "extensionPath").unwrap_or_default();
    let name = Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "an extension".to_owned());
    let error = redact_text(&string(frame, "error").unwrap_or_default());

    AgentEvent::Notice {
        level: crate::model::NoticeLevel::Error,
        message: format!("{name} failed: {error}"),
    }
}
