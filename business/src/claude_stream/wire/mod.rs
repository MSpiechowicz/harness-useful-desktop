//! Claude Code's stream-json frames and its control protocol, as serde types, shaped after the
//! recordings of Claude Code 2.1.304 (`business/tests/fixtures/claude/stream`) with the Agent SDK's
//! `sdk.d.ts` as the secondary reference.
//!
//! - [`decode`] reads one stdout line as an [`Inbound`] frame. A frame type or `system` subtype this
//!   app does not know decodes to an `Unknown` variant, and unknown fields are ignored, so a newer
//!   Claude Code never breaks the chat. Only a line that is not a JSON object, or a known frame whose
//!   shape does not fit, is a [`FrameError`] (it never carries frame content). Informational
//!   numbers (counts, durations, tokens, costs) are read leniently ([`lenient`]): a float or
//!   another value where a whole number was expected never fails the frame.
//! - [`messages`]: `assistant`, `user` and `stream_event` frames and their content blocks.
//! - [`system`]: the `system` subtypes (`init`, `status`, `compact_boundary`, `task_*`, …).
//! - [`result`]: the `result` frame that ends every turn.
//! - [`control`]: control requests the CLI sends (`can_use_tool`, `hook_callback`, …), the replies to
//!   the app's own requests and their payloads.
//! - [`outbound`]: encoders for what the app writes to stdin: user messages, control requests and
//!   control replies.

pub mod control;
pub mod messages;
pub mod outbound;
pub mod result;
pub mod system;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

pub use self::control::{CliRequest, ControlRequestFrame, ControlResponse};
pub use self::messages::{AssistantFrame, StreamEventFrame, UserFrame};
pub use self::result::ResultFrame;
pub use self::system::{System, SystemFrame};
use crate::model::AgentEvent;

/// One frame the CLI wrote to stdout.
#[derive(Debug, Clone, PartialEq)]
pub enum Inbound {
    /// A `system` frame.
    System(SystemFrame),
    /// One content block of an API message (one frame per block, all with the message's id).
    Assistant(AssistantFrame),
    /// Tool results, an interruption marker, local command output or a compaction summary.
    User(UserFrame),
    /// A partial streaming event (`--include-partial-messages`).
    StreamEvent(StreamEventFrame),
    /// The end of a turn.
    Result(Box<ResultFrame>),
    /// A request the CLI makes of the app.
    ControlRequest(ControlRequestFrame),
    /// The reply to one of the app's control requests.
    ControlResponse(ControlResponse),
    /// The CLI withdrew its request `request_id` (a permission prompt or a dialog).
    ControlCancel {
        /// The withdrawn request.
        request_id: String,
    },
    /// Where a sent user message is in the CLI's command queue.
    CommandLifecycle(CommandLifecycle),
    /// The account's rate limit state, once per turn.
    RateLimit(RateLimitEvent),
    /// Keeps the pipe alive; means nothing.
    KeepAlive,
    /// The conversation was discarded (`/clear`).
    ConversationReset(ConversationReset),
    /// A frame type this app does not know.
    Unknown {
        /// The frame's `type`.
        frame_type: String,
    },
}

impl Inbound {
    /// The session id the frame carries, when it carries one.
    pub fn session_id(&self) -> Option<&str> {
        let id = match self {
            Self::System(frame) => frame.session_id.as_deref(),
            Self::Assistant(frame) => frame.session_id.as_deref(),
            Self::User(frame) => frame.session_id.as_deref(),
            Self::StreamEvent(frame) => frame.session_id.as_deref(),
            Self::Result(frame) => frame.session_id.as_deref(),
            Self::CommandLifecycle(frame) => frame.session_id.as_deref(),
            _ => None,
        };

        id.filter(|id| !id.is_empty())
    }
}

/// Where a sent user message is in the CLI's queue (`command_lifecycle`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CommandLifecycle {
    /// The `uuid` the app gave the user message.
    #[serde(default, deserialize_with = "nullable")]
    pub command_uuid: String,
    /// What happened to it.
    #[serde(default, deserialize_with = "nullable")]
    pub state: LifecycleState,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// A `command_lifecycle` state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    /// Waiting in the queue.
    Queued,
    /// Taken by a turn.
    Started,
    /// Done (its turn may still run).
    Completed,
    /// Dropped (`cancel_async_message`, an interrupt, a failed turn).
    Cancelled,
    /// A state this app does not know.
    #[default]
    #[serde(other)]
    Other,
}

/// `rate_limit_event`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct RateLimitEvent {
    #[serde(default, deserialize_with = "nullable")]
    pub rate_limit_info: RateLimitInfo,
}

/// The account's rate limit state.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RateLimitInfo {
    /// `allowed`, `allowed_warning` or `rejected`.
    pub status: Option<String>,
    /// When the window resets, in seconds since the epoch.
    pub resets_at: Option<u64>,
    /// The window that applies (`five_hour`, `seven_day`, …).
    pub rate_limit_type: Option<String>,
}

/// `conversation_reset`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct ConversationReset {
    /// The id the conversation goes on with.
    pub new_conversation_id: Option<String>,
    /// What discarded it (`clear`, `plan_mode_exit`, …).
    pub trigger: Option<String>,
}

/// A line that is not a frame this app can read. Never holds frame content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameError {
    /// The frame's `type` (and `subtype`), when readable.
    pub frame_type: Option<String>,
    /// What went wrong.
    pub error: String,
}

impl FrameError {
    fn new(frame_type: Option<String>, error: &str) -> Self {
        Self {
            frame_type,
            error: error.to_owned(),
        }
    }

    /// The error as a chat event.
    pub fn event(&self) -> AgentEvent {
        AgentEvent::FrameError {
            original_type: self.frame_type.clone(),
            error: self.error.clone(),
        }
    }
}

/// Reads one stdout line (see the module docs).
pub fn decode(line: &str) -> Result<Inbound, FrameError> {
    let value: Value = serde_json::from_str(line).map_err(|_| FrameError::new(None, "The line is not valid JSON."))?;

    decode_value(value)
}

/// Reads one frame already parsed as JSON.
pub fn decode_value(frame: Value) -> Result<Inbound, FrameError> {
    let Some(kind) = frame.get("type").and_then(Value::as_str).map(str::to_owned) else {
        return Err(FrameError::new(None, "The frame has no type."));
    };

    let inbound = match kind.as_str() {
        "system" => Inbound::System(system::decode(frame).map_err(|subtype| shape_error(&kind, subtype))?),
        "assistant" => Inbound::Assistant(typed(&kind, frame)?),
        "user" => Inbound::User(typed(&kind, frame)?),
        "stream_event" => Inbound::StreamEvent(typed(&kind, frame)?),
        "result" => Inbound::Result(Box::new(typed(&kind, frame)?)),
        "control_request" => Inbound::ControlRequest(typed(&kind, frame)?),
        "control_response" => Inbound::ControlResponse(
            control::decode_response(frame)
                .ok_or_else(|| FrameError::new(Some(kind.clone()), "The control response has no request id."))?,
        ),
        "control_cancel_request" => Inbound::ControlCancel {
            request_id: text_field(&frame, "request_id"),
        },
        "command_lifecycle" => Inbound::CommandLifecycle(typed(&kind, frame)?),
        "rate_limit_event" => Inbound::RateLimit(typed(&kind, frame)?),
        "keep_alive" => Inbound::KeepAlive,
        "conversation_reset" => Inbound::ConversationReset(typed(&kind, frame)?),
        _ => Inbound::Unknown { frame_type: kind },
    };

    Ok(inbound)
}

/// `frame` as `T`; a shape error names only the frame type.
fn typed<T: DeserializeOwned>(kind: &str, frame: Value) -> Result<T, FrameError> {
    serde_json::from_value(frame).map_err(|_| shape_error(kind, None))
}

fn shape_error(kind: &str, subtype: Option<String>) -> FrameError {
    let frame_type = match subtype {
        Some(subtype) => format!("{kind}/{subtype}"),
        None => kind.to_owned(),
    };

    FrameError::new(Some(frame_type), "The frame does not have the expected shape.")
}

fn text_field(frame: &Value, key: &str) -> String {
    frame.get(key).and_then(Value::as_str).unwrap_or_default().to_owned()
}

/// Deserializes `null` as the type's default (Claude Code sends `null` for many absent values).
pub(crate) fn nullable<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// A number field read leniently: a whole number, or a float rounded (a negative or non-finite one,
/// a string or any other value reads as the default), so an informational number never fails a
/// frame. `null` and an absent field read as the default too.
pub(crate) fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Lenient + Default,
{
    Ok(lenient_option(deserializer)?.unwrap_or_default())
}

/// As [`lenient`], `None` for what is not a fitting number.
pub(crate) fn lenient_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Lenient,
{
    Ok(T::read(&Value::deserialize(deserializer)?))
}

/// A number type [`lenient`] reads.
pub(crate) trait Lenient: Sized {
    fn read(value: &Value) -> Option<Self>;
}

impl Lenient for f64 {
    fn read(value: &Value) -> Option<Self> {
        value.as_f64().filter(|number| number.is_finite())
    }
}

macro_rules! lenient_whole {
    ($($kind:ty),*) => {$(
        impl Lenient for $kind {
            fn read(value: &Value) -> Option<Self> {
                Self::try_from(whole(value)?).ok()
            }
        }
    )*};
}

lenient_whole!(u16, u32, u64);

/// `value` as a whole number: as sent, or a float rounded; `None` when negative, not finite or not
/// a number.
fn whole(value: &Value) -> Option<u64> {
    if let Some(number) = value.as_u64() {
        return Some(number);
    }

    let number = value.as_f64().filter(|number| number.is_finite() && *number >= 0.0)?;
    let rounded = number.round();

    // Saturates above u64::MAX, which no real count reaches.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(rounded as u64)
}
