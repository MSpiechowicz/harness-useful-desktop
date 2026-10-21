//! What the OpenCode 2 chat sends (prompt, command, shell, compact bodies), the inbox item the
//! server admits it as, and the asks it answers: permission requests and forms with their reply
//! bodies.
//!
//! Every response type reads leniently: unknown fields are ignored, missing optional ones are
//! `None` or empty.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::session::count;
use crate::opencode_server_v2::routes::MessageId;

/// When a prompt sent during a run is delivered: at the next step boundary (`steer`) or after the
/// run (`queue`). Without one the server steers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Delivery {
    /// At the next step boundary of the running execution.
    Steer,
    /// After the running execution.
    Queue,
}

/// An attached file: `{uri: "file:///abs/path", name?}`; the server reads and inlines it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileRef {
    /// A `file://` URI.
    pub uri: String,
    /// Display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl FileRef {
    /// A local file as a `file://` URI, named after its file name; `None` for a relative or
    /// non-UTF-8 path. Bytes outside RFC 3986 unreserved, `/` and `:` are percent-encoded.
    pub fn local(path: &Path) -> Option<Self> {
        if !path.is_absolute() {
            return None;
        }

        let text = path.to_str()?.replace('\\', "/");
        let text = if text.starts_with('/') {
            text
        } else {
            format!("/{text}")
        };

        Some(Self {
            uri: format!("file://{}", uri_path(&text)),
            name: path.file_name().and_then(|name| name.to_str()).map(str::to_owned),
        })
    }
}

fn uri_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());

    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/' | b':') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
}

/// An agent mentioned in a prompt (`@name`): `{name}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentMention {
    /// The agent id.
    pub name: String,
}

/// A skill attached to a prompt: `{id}`; the server adds its instructions to the message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillRef {
    /// The skill id (`GET /api/skill`).
    pub id: String,
}

/// `POST …/prompt` body: `{id?, text, files?, agents?, skills?, delivery?}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PromptBody {
    /// The id the message gets (`msg_…`), when the app names it: steering is then known by its id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<MessageId>,
    /// The prompt.
    pub text: String,
    /// Attached files.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileRef>,
    /// Mentioned agents.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<AgentMention>,
    /// Attached skills.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<SkillRef>,
    /// When a prompt sent during a run is delivered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery: Option<Delivery>,
}

impl PromptBody {
    /// A plain text prompt.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    /// The prompt with `files` attached.
    pub fn with_files(mut self, files: Vec<FileRef>) -> Self {
        self.files = files;
        self
    }

    /// The prompt named `id` (the inbox item and the message get it).
    pub fn with_id(mut self, id: MessageId) -> Self {
        self.id = Some(id);
        self
    }

    /// The prompt with its delivery during a run.
    pub fn with_delivery(mut self, delivery: Delivery) -> Self {
        self.delivery = Some(delivery);
        self
    }

    /// The prompt with skill `id` attached.
    pub fn with_skill(mut self, id: impl Into<String>) -> Self {
        self.skills.push(SkillRef { id: id.into() });
        self
    }

    /// The prompt mentioning `agent`.
    pub fn with_agent(mut self, agent: impl Into<String>) -> Self {
        self.agents.push(AgentMention { name: agent.into() });
        self
    }
}

/// `POST …/command` body: `{name, text}`; `text` holds the arguments (`""` for none).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandBody {
    /// The command name, without the slash.
    pub name: String,
    /// The arguments.
    pub text: String,
}

/// `POST …/shell` body: `{command}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShellBody {
    /// The shell command line.
    pub command: String,
}

/// `POST …/compact` body: `{}` (the server steers it in at the next boundary).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct CompactBody {}

/// An inbox item as the server admitted it (`prompt` and `compact` answers):
/// `{id, sessionID, time: {created}, type, payload, delivery}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InboxEntry {
    /// `msg_…`: the id the delivered message gets.
    pub id: String,
    /// The session.
    #[serde(default, rename = "sessionID")]
    pub session_id: String,
    /// When it was admitted.
    #[serde(default)]
    pub time: InboxTime,
    /// The item.
    #[serde(flatten)]
    pub item: InboxItem,
}

/// When an inbox item was admitted, in milliseconds since the epoch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct InboxTime {
    /// Admitted.
    #[serde(default, deserialize_with = "count")]
    pub created: u64,
}

/// An inbox item: `{type: user|synthetic|compaction|move, payload, delivery}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InboxItem {
    /// `user`, `synthetic` (a shell command's output, …), `compaction` or `move`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The text and files.
    #[serde(default)]
    pub payload: InboxPayload,
    /// `steer` or `queue`.
    #[serde(default)]
    pub delivery: Option<Delivery>,
}

/// An inbox item's payload; empty for a compaction.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct InboxPayload {
    /// The prompt or synthetic text.
    #[serde(default)]
    pub text: Option<String>,
    /// A synthetic item's short description (a shell command line).
    #[serde(default)]
    pub description: Option<String>,
    /// Attached files, as the server stored them.
    #[serde(default)]
    pub files: Vec<Value>,
    /// Attached skills (`{id, name, text?}`), as the server stored them.
    #[serde(default)]
    pub skills: Vec<Value>,
}

/// A pending permission request (`permission.asked`, `GET …/permission`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PermissionRequest {
    /// `per_…`.
    pub id: String,
    /// The session.
    #[serde(default, rename = "sessionID")]
    pub session_id: String,
    /// The permission (`edit`, `shell`, `read`, …).
    #[serde(default)]
    pub action: String,
    /// What it is asked for (file paths, command lines).
    #[serde(default)]
    pub resources: Vec<String>,
    /// The patterns an `always` reply saves.
    #[serde(default)]
    pub save: Vec<String>,
    /// Details for the card (`files: [{file, patch, …}]` for edits).
    #[serde(default)]
    pub metadata: Value,
    /// The tool call that asks.
    #[serde(default)]
    pub source: Option<PermissionSource>,
    /// A note from the asker.
    #[serde(default)]
    pub message: Option<String>,
}

/// The tool call behind a permission request: `{type: tool, messageID, id}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PermissionSource {
    /// `tool`.
    #[serde(default, rename = "type")]
    pub kind: String,
    /// The assistant message.
    #[serde(default, rename = "messageID")]
    pub message_id: String,
    /// The tool call id.
    #[serde(default)]
    pub id: String,
}

/// A permission reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// Allow this request.
    Once,
    /// Allow this request and save its `save` patterns.
    Always,
    /// Refuse it.
    Reject,
}

/// `POST …/permission/{id}/reply` body: `{decision, message?}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PermissionReplyBody {
    /// The reply.
    pub decision: Decision,
    /// Why, for a rejection; the model reads it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// A form (`form.created`, `GET /api/form`): the questions a tool asks.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Form {
    /// `frm_…`.
    pub id: String,
    /// The session (`global` for a form no session owns).
    #[serde(default, rename = "sessionID")]
    pub session_id: String,
    /// The heading.
    #[serde(default)]
    pub title: String,
    /// `{kind: "question", tool: {messageID, id}}` for the question tool.
    #[serde(default)]
    pub metadata: Value,
    /// The questions.
    #[serde(default)]
    pub fields: Vec<FormField>,
}

/// One form field.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FormField {
    /// The answer key (`q0`, …).
    pub key: String,
    /// `string`, `number`, `integer`, `boolean`, `multiselect` or `external`.
    #[serde(default, rename = "type")]
    pub kind: String,
    /// The question.
    #[serde(default)]
    pub title: Option<String>,
    /// More detail.
    #[serde(default)]
    pub description: Option<String>,
    /// Must be answered.
    #[serde(default)]
    pub required: bool,
    /// The choices.
    #[serde(default)]
    pub options: Vec<FormOption>,
    /// Free text is accepted besides the choices.
    #[serde(default)]
    pub custom: bool,
}

/// A choice: `{value, label, description?}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FormOption {
    /// The answer value.
    pub value: String,
    /// What is shown.
    #[serde(default)]
    pub label: String,
    /// More detail.
    #[serde(default)]
    pub description: Option<String>,
}

/// One answer value: text, number, yes/no or several choices.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum FormValue {
    /// A string field.
    Text(String),
    /// A number or integer field.
    Number(f64),
    /// A boolean field.
    Bool(bool),
    /// A multiselect field.
    Many(Vec<String>),
}

/// `POST …/form/{id}/reply` body: `{answer: {key: value}}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct FormReplyBody {
    /// The answers by field key.
    pub answer: BTreeMap<String, FormValue>,
}
