//! What Harness Knowledge's database holds (`knowledge_stats`), read from `harness-knowledge stats
//! --json`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
}
