//! The models and agents an OpenCode 2 chat offers, read once at startup (`GET /api/model`,
//! `GET /api/agent`).
//!
//! - **Models:** every enabled model, with its display fields; its variants are the thinking
//!   levels, [`DEFAULT_VARIANT`] first. A model with variants counts as a reasoning model (OpenCode
//!   2 lists no reasoning flag).
//! - **Agents:** the primary ones (`primary` or `all`, not hidden). OpenCode 2.1.2 lists a
//!   directory's agents only once it loaded them, 150 to 300 ms after the first request there
//!   (startup asks again meanwhile, `startup.rs`). When the list stays empty, the chat offers
//!   [`FALLBACK_AGENTS`], the primary agents every install has, so the agent picker still works.

use crate::model::{AgentOption, ModelOption};
use crate::opencode_server_v2::wire::{Agent, Model, ModelRef};

/// The thinking level that selects no model variant.
pub(super) const DEFAULT_VARIANT: &str = "default";

/// OpenCode 2's own primary agents (`id`, name), offered when the server lists none.
const FALLBACK_AGENTS: [(&str, &str); 2] = [("build", "Build"), ("plan", "Plan")];

/// The models and agents the server offered at startup.
pub(super) struct Catalog {
    pub models: Vec<CatalogModel>,
    pub agents: Vec<AgentOption>,
}

/// A model of the catalog.
pub(super) struct CatalogModel {
    /// The display fields.
    pub option: ModelOption,
    /// Variant ids, in the server's order.
    pub variants: Vec<String>,
}

impl CatalogModel {
    /// True for the model `model` names (any variant).
    fn is(&self, model: &ModelRef) -> bool {
        self.option.provider == model.provider_id && self.option.id == model.id
    }
}

impl Catalog {
    /// The catalog of the server's `models` and `agents`.
    pub(super) fn new(models: Vec<Model>, agents: &[Agent]) -> Self {
        let models = models
            .into_iter()
            .filter(|model| model.enabled)
            .map(catalog_model)
            .collect();

        let mut agents: Vec<AgentOption> = agents
            .iter()
            .filter(|agent| agent.is_primary())
            .map(|agent| AgentOption {
                id: agent.id.clone(),
                name: if agent.name.is_empty() {
                    agent.id.clone()
                } else {
                    agent.name.clone()
                },
                description: agent.description.clone(),
            })
            .collect();

        if agents.is_empty() {
            agents = FALLBACK_AGENTS
                .iter()
                .map(|(id, name)| AgentOption {
                    id: (*id).to_owned(),
                    name: (*name).to_owned(),
                    description: None,
                })
                .collect();
        }

        Self { models, agents }
    }

    /// The catalog entry of `model`.
    pub(super) fn model(&self, model: &ModelRef) -> Option<&CatalogModel> {
        self.models.iter().find(|candidate| candidate.is(model))
    }

    /// The variants of `model` with [`DEFAULT_VARIANT`] first; empty without variants.
    pub(super) fn levels(&self, model: Option<&ModelRef>) -> Vec<String> {
        let variants = model.and_then(|model| self.model(model)).map(|model| &model.variants);

        match variants {
            Some(variants) if !variants.is_empty() => std::iter::once(DEFAULT_VARIANT.to_owned())
                .chain(variants.iter().cloned())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The display fields of `model`; just its ids when the server did not list it.
    pub(super) fn option(&self, model: &ModelRef) -> ModelOption {
        match self.model(model) {
            Some(known) => known.option.clone(),
            None => ModelOption {
                provider: model.provider_id.clone(),
                id: model.id.clone(),
                name: None,
                reasoning: false,
                context_window: None,
                max_tokens: None,
                input: Vec::new(),
            },
        }
    }

    /// The agent the session uses when the chat can switch to it, else the first one offered.
    pub(super) fn chat_agent(&self, last: Option<&str>) -> String {
        let known = |agent: &&str| self.agents.iter().any(|option| option.id == *agent);

        last.filter(known)
            .map(str::to_owned)
            .or_else(|| self.agents.first().map(|agent| agent.id.clone()))
            .unwrap_or_else(|| FALLBACK_AGENTS[0].0.to_owned())
    }
}

fn catalog_model(model: Model) -> CatalogModel {
    let variants: Vec<String> = model
        .variants
        .into_iter()
        .map(|variant| variant.id)
        .filter(|variant| variant != DEFAULT_VARIANT)
        .collect();

    let option = ModelOption {
        provider: model.provider_id,
        id: model.id,
        name: Some(model.name).filter(|name| !name.is_empty()),
        reasoning: !variants.is_empty(),
        context_window: Some(model.limit.context).filter(|window| *window > 0),
        max_tokens: Some(model.limit.output).filter(|tokens| *tokens > 0),
        input: model.capabilities.input,
    };

    CatalogModel { option, variants }
}
