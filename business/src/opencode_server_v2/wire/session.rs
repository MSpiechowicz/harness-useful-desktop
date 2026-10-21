//! The OpenCode 2 session object (`id`, `title`, `time`, `location`, `cost`, `tokens`), the model
//! reference, token counts and errors it shares with the events, and the session request bodies.
//!
//! Every type reads leniently: unknown fields are ignored, missing optional ones are `None` or 0.

use serde::{Deserialize, Deserializer, Serialize};

use super::envelope::Location;
use crate::opencode_server_v2::routes::MessageId;

/// `Session.Info` as the server answers it (`GET`/`POST /api/session`, fork). `title` is absent
/// until the session is named.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Session {
    /// `ses_…`.
    pub id: String,
    /// The project (`global` outside a repository).
    #[serde(default, rename = "projectID")]
    pub project_id: String,
    /// The session's name, once it has one.
    #[serde(default)]
    pub title: Option<String>,
    /// The parent of a child session.
    #[serde(default, rename = "parentID")]
    pub parent_id: Option<String>,
    /// Where a forked session came from.
    #[serde(default)]
    pub fork: Option<ForkInfo>,
    /// The selected agent, once one was chosen.
    #[serde(default)]
    pub agent: Option<String>,
    /// The selected model.
    #[serde(default)]
    pub model: Option<ModelRef>,
    /// Total cost in USD.
    #[serde(default)]
    pub cost: f64,
    /// Total token usage.
    #[serde(default)]
    pub tokens: Tokens,
    /// Outcome of the last finished execution (`succeeded`, `failed`, `interrupted`).
    #[serde(default)]
    pub outcome: Option<String>,
    /// Timestamps.
    #[serde(default)]
    pub time: SessionTime,
    /// The session's project directory.
    #[serde(default)]
    pub location: Location,
}

/// A session's timestamps, in milliseconds since the epoch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct SessionTime {
    /// Created.
    #[serde(default, deserialize_with = "count")]
    pub created: u64,
    /// Last changed.
    #[serde(default, deserialize_with = "count")]
    pub updated: u64,
    /// Last became idle.
    #[serde(default, deserialize_with = "optional_count")]
    pub idle: Option<u64>,
}

/// Where a forked session came from: `{sessionID, boundary}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ForkInfo {
    /// The parent session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// Where the copy stops.
    pub boundary: ForkBoundary,
}

/// A fork boundary: `{type: before|through, messageID}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ForkBoundary {
    /// `before` (history before the message) or `through` (the whole history).
    #[serde(rename = "type")]
    pub kind: String,
    /// The boundary message.
    #[serde(default, rename = "messageID")]
    pub message_id: Option<String>,
}

/// A model by provider and id, with its variant (thinking level): `{id, providerID, variant?}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelRef {
    /// Model id within the provider.
    pub id: String,
    /// Provider id.
    #[serde(rename = "providerID")]
    pub provider_id: String,
    /// The variant (`default`, `low`, `high`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Token usage: `{input, output, reasoning, cache: {read, write}}`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Tokens {
    /// Input tokens.
    #[serde(default, deserialize_with = "count")]
    pub input: u64,
    /// Output tokens.
    #[serde(default, deserialize_with = "count")]
    pub output: u64,
    /// Reasoning tokens.
    #[serde(default, deserialize_with = "count")]
    pub reasoning: u64,
    /// Cache reads and writes.
    #[serde(default)]
    pub cache: CacheTokens,
}

/// Cached tokens: `{read, write}`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct CacheTokens {
    /// Read from the cache.
    #[serde(default, deserialize_with = "count")]
    pub read: u64,
    /// Written to the cache.
    #[serde(default, deserialize_with = "count")]
    pub write: u64,
}

/// A structured error (`Session.StructuredError`): `{type, message}`; `type` is `aborted` for an
/// interruption, `permission.rejected` for a refused permission.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ErrorInfo {
    /// The error kind.
    #[serde(default, rename = "type")]
    pub kind: String,
    /// What went wrong.
    #[serde(default)]
    pub message: String,
}

/// `POST /api/session` body: `{title?, agent?, model?}`. Never carries permissions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CreateBody {
    /// The session's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// The model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
}

/// `PATCH /api/session/{id}` body: the title and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TitleBody {
    /// The new name.
    pub title: String,
}

/// `POST …/fork` body: `{before?}`. Without `before` the whole history is copied.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ForkBody {
    /// Copy the history before this message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<MessageId>,
}

/// `POST …/agent` body: `{agent}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentBody {
    /// The agent id.
    pub agent: String,
}

/// `POST …/model` body: `{model: {id, providerID, variant?}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelBody {
    /// The model.
    pub model: ModelRef,
}

/// A JSON number as a count: OpenCode writes `Finite` numbers, so a fraction or a negative value is
/// rounded and clamped instead of failing the whole object.
pub(crate) fn count<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let number = serde_json::Number::deserialize(deserializer)?;

    Ok(number_to_count(&number))
}

pub(crate) fn optional_count<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    let number = Option::<serde_json::Number>::deserialize(deserializer)?;

    Ok(number.as_ref().map(number_to_count))
}

fn number_to_count(number: &serde_json::Number) -> u64 {
    if let Some(value) = number.as_u64() {
        return value;
    }

    // Saturating: `as` clamps a float to 0..=u64::MAX and maps NaN to 0.
    number.as_f64().map_or(0, |value| value.round() as u64)
}
