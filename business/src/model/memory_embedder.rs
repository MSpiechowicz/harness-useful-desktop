//! Vector search in Harness Memory: the local embedder the app keeps running
//! (`memory_embedder_watch`), the switch for it (`memory_set_vector_search`) and the English notes
//! setting (`memory_settings_get`, `memory_settings_set`).
//!
//! The embedder itself is `harness-memory embedder run`, see `crate::ecosystem::memory_embedder`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where the embedder is in its life.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EmbedderPhase {
    /// Not running: vector search is off, or Harness Memory cannot run it.
    #[default]
    Off,
    /// The process started and has not said anything yet.
    Starting,
    /// Downloading the model (`progress` says how far).
    Downloading,
    /// Checking the downloaded model.
    Verifying,
    /// Loading the model.
    Loading,
    /// Serving searches.
    Ready,
    /// Embedding the notes already saved (`progress` says how far).
    Backfilling,
    /// Moving to a newer model while the old one still serves (`progress` says how far).
    Updating,
    /// It stopped and will not be restarted by itself (it kept crashing, or it reported an error).
    Failed,
    /// Another app instance runs the one embedder of this computer.
    Elsewhere,
}

/// What the embedder is doing now, as `memory_embedder_watch` sends it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryEmbedderState {
    /// Where it is in its life.
    pub phase: EmbedderPhase,
    /// How far the download, the backfill or the move to a newer model is, from 0 to 1; `None`
    /// when nothing is in progress or the total is not known.
    pub progress: Option<f64>,
    /// The model it serves, once it said so.
    pub model: Option<String>,
    /// The last error it reported, or why it was given up (redacted, clipped).
    pub error: Option<String>,
}

/// The answer of `memory_set_vector_search`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryVectorSwitchResult {
    /// The stored `vectorSearch` pref.
    pub vector_search: bool,
    /// The embedder right after the switch.
    pub state: MemoryEmbedderState,
}

/// The settings Harness Memory keeps in the store (`harness-memory settings get --json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemorySettings {
    /// Whether agents must write notes in English (on unless set otherwise, team-wide on a team
    /// database).
    pub english_notes: bool,
}
