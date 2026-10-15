//! Command encoding and reply parsing shared by the PI-family dialects.
//!
//! OMP and PI accept the same command names and parameters for most actions; the dialects handle
//! the rest (`clone`, the command list, history) themselves.

use serde_json::{Value, json};

use super::{EncodeContext, command_frame};
use crate::error::{BusinessError, Result};
use crate::harness::CommandCatalog;
use crate::model::{AgentAction, AgentReply, BashOutput, ChatMessage, HarnessId, ImageAttachment, StreamingBehavior};
use crate::pi_family::normalize::{model_option, session_state, session_stats, string, strings};
use crate::pi_family::sessions::find_session;
use crate::security::redact_text;

/// Largest `bash` output passed to the webview (1 MiB).
pub const MAX_BASH_OUTPUT_BYTES: usize = 1024 * 1024;

fn text(value: &str) -> Value {
    Value::from(value)
}

/// The frame for an action both OMP and PI encode the same way; `None` for the rest.
pub fn encode_shared(
    harness: HarnessId,
    action: &AgentAction,
    id: &str,
    context: &EncodeContext<'_>,
) -> Result<Option<Value>> {
    use AgentAction as A;

    let frame = |kind: &str, params: Vec<(&str, Value)>| Some(command_frame(kind, id, params));

    let encoded = match action {
        A::Prompt {
            message,
            images,
            streaming_behavior,
        } => frame(
            "prompt",
            with_images(
                vec![
                    ("message", text(message)),
                    ("streamingBehavior", streaming_behavior.map_or(Value::Null, behavior)),
                ],
                images,
            ),
        ),
        A::Steer { message, images } => frame("steer", with_images(vec![("message", text(message))], images)),
        A::FollowUp { message, images } => frame("follow_up", with_images(vec![("message", text(message))], images)),
        A::Abort => frame("abort", Vec::new()),
        A::NewSession => frame("new_session", Vec::new()),
        A::SwitchSession { session } => {
            if session.harness != harness {
                return Err(BusinessError::InvalidInput(
                    "the session belongs to another harness".to_owned(),
                ));
            }

            let path = find_session(context.sessions_dir, &session.id)?;
            frame("switch_session", vec![("sessionPath", text(&path.to_string_lossy()))])
        }
        A::Fork { entry_id } => frame("fork", vec![("entryId", text(entry_id))]),
        A::SetModel { provider, model_id } => frame(
            "set_model",
            vec![("provider", text(provider)), ("modelId", text(model_id))],
        ),
        A::SetThinkingLevel { level } => frame("set_thinking_level", vec![("level", text(level))]),
        A::Compact { instructions } => frame(
            "compact",
            vec![("customInstructions", instructions.as_deref().map_or(Value::Null, text))],
        ),
        A::SetSessionName { name } => frame("set_session_name", vec![("name", text(name))]),
        A::GetState => frame("get_state", Vec::new()),
        A::GetAvailableModels => frame("get_available_models", Vec::new()),
        A::GetAvailableThinkingLevels => frame("get_available_thinking_levels", Vec::new()),
        A::GetSessionStats => frame("get_session_stats", Vec::new()),
        A::ExportHtml => frame("export_html", Vec::new()),
        A::GetLastAssistantText => frame("get_last_assistant_text", Vec::new()),
        A::Clone | A::RefreshCommands | A::GetHistory { .. } | A::RemoveQueued { .. } => None,
    };

    Ok(encoded)
}

/// `params` plus the `images` parameter, which is left out when there are none.
fn with_images<'a>(mut params: Vec<(&'a str, Value)>, images: &[ImageAttachment]) -> Vec<(&'a str, Value)> {
    if !images.is_empty() {
        let blocks = images
            .iter()
            .map(|image| json!({ "type": "image", "data": image.data, "mimeType": image.mime_type }))
            .collect();
        params.push(("images", Value::Array(blocks)));
    }

    params
}

fn behavior(behavior: StreamingBehavior) -> Value {
    match behavior {
        StreamingBehavior::Steer => text("steer"),
        StreamingBehavior::FollowUp => text("followUp"),
    }
}

/// The typed reply to `action` from its response `data`.
pub fn parse_reply(action: &AgentAction, data: Option<&Value>, catalog: &dyn CommandCatalog) -> Result<AgentReply> {
    use AgentAction as A;

    let null = Value::Null;
    let data = data.unwrap_or(&null);

    let reply = match action {
        A::Prompt { .. } => AgentReply::Prompt {
            agent_invoked: data.get("agentInvoked").and_then(Value::as_bool),
            disposition: string(data, "disposition"),
        },
        A::GetState => AgentReply::State {
            state: session_state(data),
        },
        A::GetAvailableModels => AgentReply::Models {
            models: items(data, "models").iter().filter_map(model_option).collect(),
        },
        A::GetAvailableThinkingLevels => AgentReply::ThinkingLevels {
            levels: strings(data, "levels"),
        },
        A::GetSessionStats => AgentReply::Stats {
            stats: session_stats(data),
        },
        A::ExportHtml => AgentReply::Exported {
            path: string(data, "path")
                .ok_or_else(|| BusinessError::Protocol("export_html returned no path".to_owned()))?,
        },
        A::GetLastAssistantText => AgentReply::Text {
            text: string(data, "text"),
        },
        A::RefreshCommands => AgentReply::Commands {
            commands: catalog.merged(&items(data, "commands")),
        },
        A::RemoveQueued { .. } => AgentReply::QueueRemoval {
            removed: data.get("removed").and_then(Value::as_bool).unwrap_or(false),
        },
        _ => AgentReply::Ack,
    };

    Ok(reply)
}

/// The array at `key` (empty when absent).
pub fn items(data: &Value, key: &str) -> Vec<Value> {
    data.get(key).and_then(Value::as_array).cloned().unwrap_or_default()
}

/// A `bash` result: redacted and capped at [`MAX_BASH_OUTPUT_BYTES`].
pub fn parse_bash(data: Option<&Value>) -> BashOutput {
    let null = Value::Null;
    let data = data.unwrap_or(&null);
    let mut output = redact_text(&string(data, "output").unwrap_or_default());
    let mut truncated = data.get("truncated").and_then(Value::as_bool).unwrap_or(false);

    if output.len() > MAX_BASH_OUTPUT_BYTES {
        let mut end = MAX_BASH_OUTPUT_BYTES;
        while !output.is_char_boundary(end) {
            end -= 1;
        }

        output.truncate(end);
        truncated = true;
    }

    BashOutput {
        output,
        exit_code: data
            .get("exitCode")
            .and_then(Value::as_i64)
            .and_then(|code| i32::try_from(code).ok()),
        cancelled: data.get("cancelled").and_then(Value::as_bool).unwrap_or(false),
        truncated,
    }
}

/// The newest-first page of `messages` ending before `cursor` (an offset from
/// [`AgentReply::History::next_cursor`]); messages inside the page stay oldest first.
pub fn slice_history(messages: Vec<ChatMessage>, cursor: Option<&str>, limit: u32) -> Result<AgentReply> {
    let total = messages.len();

    let end = match cursor {
        None => total,
        Some(raw) => raw
            .parse::<usize>()
            .ok()
            .filter(|end| *end <= total)
            .ok_or_else(|| BusinessError::InvalidInput("unknown history cursor".to_owned()))?,
    };

    let start = end.saturating_sub(limit as usize);
    let page = messages.into_iter().skip(start).take(end - start).collect();

    Ok(AgentReply::History {
        messages: page,
        next_cursor: (start > 0).then(|| start.to_string()),
        total: u32::try_from(total).ok(),
    })
}
