//! What Harness Knowledge's database holds (`knowledge_stats`), read from `harness-knowledge stats
//! --json`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::lenient_name;

/// What the database holds (`harness-knowledge stats --json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeStats {
    /// How many documents.
    pub documents: u32,
    /// How many chunks.
    pub chunks: u32,
    /// How many chunks have a vector.
    pub embedded_chunks: u32,
    /// The size of all their text in bytes.
    #[ts(type = "number")]
    pub bytes: u64,
    /// How many documents link to no repository.
    pub global_documents: u32,
    /// How many repositories have documents.
    pub repositories: u32,
    /// Changes whenever the database changes (the live view reloads on it).
    #[ts(type = "number")]
    pub generation: i64,
    /// The schema version of the database.
    pub schema: u32,
    /// The model that embedded the chunks, once any were.
    pub model: Option<String>,
    /// How many proposals wait in the review queue. Absent from a server that predates it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub held: Option<u32>,
    /// The counts of each place in use, only when both are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub places: Option<KnowledgeStatsPlaces>,
    /// The remote database cannot be reached: the counts are this computer's.
    #[serde(default)]
    pub offline: bool,
    /// Whether the user may change the remote place: `Some(false)` for a read-only role, `None`
    /// while unknown (no remote, a remote that is down, a store that cannot tell, a server
    /// that does not say). Actions stay enabled while unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub remote_writable: Option<bool>,
    /// How many documents of this place are drafts (never agent-visible). Absent from a server
    /// without `docs-drafts`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub drafts: Option<u32>,
    /// How many documents wait for approval in every place in use. Absent as for `drafts`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub waiting: Option<u32>,
    /// How many of those waiting documents this person could approve (not written by them nor
    /// asked for by them: the sidecar's `selfApproval` rule, by their git `user.name`, `0` without
    /// one). Absent from a server that predates it, which the pulse reads as `0` so that nobody is
    /// badged for their own documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub waiting_for_you: Option<u32>,
    /// How many live documents have a pending version. Absent as for `drafts`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub pending: Option<u32>,
    /// Whether team approval is on: `None` with no team database, while it cannot be reached and
    /// from a server without `docs-approval`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub approval: Option<bool>,
    /// Who the person at the terminal is (`me`): the git `user.name` the server reads in the folder
    /// it runs in. Absent from a server that predates it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub me: Option<KnowledgeViewer>,
}

/// The person using the app, as the server knows them (`stats --json` `me`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeViewer {
    /// Their git `user.name`, none when no name is set.
    #[serde(default, with = "lenient_name")]
    #[ts(optional = nullable, as = "Option<String>")]
    pub name: Option<String>,
}

/// The counts of each place in use (`stats --json` `places`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeStatsPlaces {
    /// The team database.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub remote: Option<KnowledgePlaceStats>,
    /// This computer's store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub local: Option<KnowledgePlaceStats>,
}

/// What one place holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgePlaceStats {
    /// How many documents.
    #[serde(default)]
    pub documents: u32,
    /// How many chunks.
    #[serde(default)]
    pub chunks: u32,
    /// How many chunks have a vector.
    #[serde(default)]
    pub embedded_chunks: u32,
    /// How many documents link to no repository.
    #[serde(default)]
    pub global_documents: u32,
    /// Changes whenever this place changes (0 from a server that does not count it).
    #[serde(default)]
    #[ts(type = "number")]
    pub generation: i64,
    /// The place cannot be reached, so its counts are not known (`{"offline": true}`).
    #[serde(default)]
    pub offline: bool,
    /// How many documents are drafts (absent from a server without `docs-drafts`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub drafts: Option<u32>,
    /// How many documents wait for approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub waiting: Option<u32>,
    /// How many live documents have a pending version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub pending: Option<u32>,
}
