//! The OpenCode 2 agent, command, skill and model lists.
//!
//! Nothing is copied wholesale: agents keep their id, name, description and mode (not their
//! request headers or system prompt), skills their id, name and description (not their path or
//! instructions), models their display fields (not `settings`, which hold API
//! keys and URLs). Every type reads leniently.

use serde::Deserialize;

use super::session::{ModelRef, count};

/// An agent of `GET /api/agent`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Agent {
    /// The agent id (`build`, `plan`, `general`, …).
    pub id: String,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// What the agent is for.
    #[serde(default)]
    pub description: Option<String>,
    /// `primary`, `subagent` or `all`.
    #[serde(default)]
    pub mode: String,
    /// Hidden from pickers.
    #[serde(default)]
    pub hidden: bool,
    /// The agent's own model, if it pins one.
    #[serde(default)]
    pub model: Option<ModelRef>,
}

impl Agent {
    /// True for an agent the user can pick for a chat: `primary` or `all`, not hidden.
    pub fn is_primary(&self) -> bool {
        !self.hidden && matches!(self.mode.as_str(), "primary" | "all")
    }
}

/// A slash command of `GET /api/command`: `{name, description?}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Command {
    /// The name, without the slash.
    pub name: String,
    /// What it does.
    #[serde(default)]
    pub description: Option<String>,
}

/// A skill of `GET /api/skill`: `{id, name, description?}`. Its path and instructions are not
/// kept.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Skill {
    /// The id a prompt attaches it by (`skills: [{id}]`).
    pub id: String,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// What it is for.
    #[serde(default)]
    pub description: Option<String>,
}

/// A model of `GET /api/model`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Model {
    /// The model's id within its provider, as used in [`ModelRef::id`].
    pub id: String,
    /// The provider's own model id.
    #[serde(default, rename = "modelID")]
    pub model_id: String,
    /// Provider id.
    #[serde(rename = "providerID")]
    pub provider_id: String,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// Model family.
    #[serde(default)]
    pub family: Option<String>,
    /// What the model takes and gives.
    #[serde(default)]
    pub capabilities: Capabilities,
    /// The variants (thinking levels); empty when the model has none.
    #[serde(default)]
    pub variants: Vec<Variant>,
    /// Token limits.
    #[serde(default)]
    pub limit: Limit,
    /// `alpha`, `beta`, `deprecated` or `active`.
    #[serde(default)]
    pub status: Option<String>,
    /// False when the user turned the model off.
    #[serde(default = "enabled")]
    pub enabled: bool,
}

impl Model {
    /// The reference that selects this model.
    pub fn reference(&self) -> ModelRef {
        ModelRef {
            id: self.id.clone(),
            provider_id: self.provider_id.clone(),
            variant: None,
        }
    }
}

fn enabled() -> bool {
    true
}

/// A model's capabilities: `{tools, input, output}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Capabilities {
    /// Calls tools.
    #[serde(default)]
    pub tools: bool,
    /// Input kinds (`text`, `image`, …).
    #[serde(default)]
    pub input: Vec<String>,
    /// Output kinds.
    #[serde(default)]
    pub output: Vec<String>,
}

impl Capabilities {
    /// True when the model reads images.
    pub fn takes_images(&self) -> bool {
        self.input.iter().any(|kind| kind == "image")
    }
}

/// A model variant: `{id, …overlays}`; only the id is kept.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Variant {
    /// The variant id (`low`, `high`, `max`, …).
    pub id: String,
}

/// A model's token limits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Limit {
    /// Context window.
    #[serde(default, deserialize_with = "count")]
    pub context: u64,
    /// Largest output.
    #[serde(default, deserialize_with = "count")]
    pub output: u64,
}
