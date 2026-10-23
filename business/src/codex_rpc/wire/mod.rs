//! Codex app-server frames as serde types, shaped after the recordings of Codex 0.154.0
//! (`business/tests/fixtures/codex/app_server`) with the bindings `codex app-server generate-ts`
//! writes as the secondary reference. Only what the chat engine and the Sessions view use.
//!
//! - Frames are JSON-RPC without the `jsonrpc` field: requests `{id, method, params}`, responses
//!   `{id, result | error}`, notifications `{method, params}`. Request ids are numbers or strings
//!   ([`RequestId`]); Codex numbers its own requests from 0, apart from the app's.
//! - [`decode`] classifies one stdout line as an [`Inbound`] frame. A method this app does not
//!   know decodes to an `Unknown` variant, an item type it does not know to
//!   [`ThreadItem::Unknown`](items::ThreadItem::Unknown), unknown fields are ignored and missing
//!   optional ones default, so a newer Codex never breaks the chat. A known notification whose
//!   params do not fit is a [`FrameError`] (it never carries frame content). A server request
//!   whose params do not fit stays answerable: it decodes as
//!   [`ServerRequestKind::Unknown`](requests::ServerRequestKind::Unknown) with its id.
//! - [`notifications`]: what Codex reports (turns, items, deltas, tokens, names, errors, …).
//! - [`items`]: the thread items (messages, reasoning, commands, file changes, tool calls, …).
//! - [`requests`]: what Codex asks the app (approvals, questions, elicitations, …).
//! - [`outbound`]: the app's requests.
//! - [`answers`]: the `initialized` notification and the answers to Codex's requests.
//! - [`results`]: the results of the app's requests (threads, turns, models, account, …).

pub mod answers;
pub mod items;
pub mod notifications;
pub mod outbound;
pub mod requests;
pub mod results;

use std::fmt;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use self::items::ThreadItem;
pub use self::notifications::Notification;
pub use self::requests::{ServerRequest, ServerRequestKind};
use crate::model::AgentEvent;

/// The id of a request, either side's: a number or a string, answered as it came.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    Text(String),
}

impl fmt::Display for RequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(number) => write!(formatter, "{number}"),
            Self::Text(text) => formatter.write_str(text),
        }
    }
}

/// One frame Codex wrote to stdout.
#[derive(Debug, Clone, PartialEq)]
pub enum Inbound {
    /// The answer to one of the app's requests.
    Response(Response),
    /// Something Codex reports.
    Notification(Notification),
    /// Something Codex asks the app; answer it under its id.
    Request(ServerRequest),
}

/// The answer to one of the app's requests.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub id: RequestId,
    /// The `result`, or the `error`.
    pub outcome: Result<Value, RpcError>,
}

impl Response {
    /// The result as `T`; an error response or a result that does not fit fails.
    pub fn result<T: DeserializeOwned>(&self) -> Result<T, RpcError> {
        let value = self.outcome.clone()?;

        serde_json::from_value(value).map_err(|_| RpcError {
            code: INVALID_RESULT,
            message: "The result does not have the expected shape.".to_owned(),
            data: None,
        })
    }
}

/// The `code` of [`RpcError`] for a result that does not have the expected shape.
pub const INVALID_RESULT: i64 = -32001;

/// The JSON-RPC code of a request Codex rejected as invalid (`no active turn to steer`, …).
pub const INVALID_REQUEST: i64 = -32600;

/// An `error` response.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

/// A line that is not a frame this app can read. Never holds frame content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameError {
    /// The frame's `method`, when readable.
    pub method: Option<String>,
    /// What went wrong.
    pub error: String,
}

impl FrameError {
    fn new(method: Option<&str>, error: &str) -> Self {
        let method = method.map(str::to_owned);

        Self {
            error: error.to_owned(),
            method,
        }
    }

    /// The error as a chat event.
    pub fn event(&self) -> AgentEvent {
        let Self { method, error } = self.clone();

        AgentEvent::FrameError {
            original_type: method,
            error,
        }
    }
}

/// Reads one stdout line (see the module docs).
pub fn decode(line: &str) -> Result<Inbound, FrameError> {
    match serde_json::from_str::<Value>(line) {
        Ok(frame) => decode_value(frame),
        Err(_) => Err(FrameError::new(None, "The line is not valid JSON.")),
    }
}

/// Reads one frame already parsed as JSON.
pub fn decode_value(frame: Value) -> Result<Inbound, FrameError> {
    let Value::Object(mut fields) = frame else {
        return Err(FrameError::new(None, "The frame is not a JSON object."));
    };

    let method = fields.get("method").and_then(Value::as_str).map(str::to_owned);
    let id = fields
        .remove("id")
        .filter(|id| !id.is_null())
        .map(serde_json::from_value::<RequestId>);
    let params = fields.remove("params").unwrap_or(Value::Null);

    match (method, id) {
        (Some(method), Some(Ok(id))) => Ok(Inbound::Request(requests::decode(id, &method, params))),
        (Some(method), None) => notifications::decode(&method, params)
            .map(Inbound::Notification)
            .map_err(|()| FrameError::new(Some(&method), "The notification does not have the expected shape.")),
        (None, Some(Ok(id))) => Ok(Inbound::Response(response(
            id,
            fields.remove("result"),
            fields.remove("error"),
        ))),
        (method, Some(Err(_))) => Err(FrameError::new(
            method.as_deref(),
            "The frame's id is neither a number nor a string.",
        )),
        (None, None) => Err(FrameError::new(
            None,
            "The frame is neither a request, a response nor a notification.",
        )),
    }
}

fn response(id: RequestId, result: Option<Value>, error: Option<Value>) -> Response {
    let outcome = match error.filter(|error| !error.is_null()) {
        Some(error) => Err(serde_json::from_value(error).unwrap_or_else(|_| RpcError {
            message: "Codex reported an error.".to_owned(),
            ..RpcError::default()
        })),
        None => Ok(result.unwrap_or(Value::Null)),
    };

    Response { id, outcome }
}

/// `params` as `T`, `Err(())` when they do not fit (`null` reads as an empty object).
pub(crate) fn params<T: DeserializeOwned>(params: Value) -> Result<T, ()> {
    let params = if params.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        params
    };

    serde_json::from_value(params).map_err(|_| ())
}
