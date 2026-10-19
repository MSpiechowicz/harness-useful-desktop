//! The JSON `opencode serve` (1.19.3) sends and takes, read field by field (recordings in
//! `tests/fixtures/opencode/server`). Pure: no I/O.
//!
//! Nothing is copied wholesale: model entries keep their display fields (provider options, headers
//! and API URLs can hold credentials), agents their id, name and description.

use std::net::SocketAddr;

use serde_json::{Map, Value, json};

use crate::model::{AgentOption, ImageAttachment, ModelOption};

/// What `opencode serve` prints once it listens (its `serve` command).
const LISTENING_PREFIX: &str = "opencode server listening on http://";

/// Title prefixes of sessions OpenCode has not named yet (`session/session.ts`).
const DEFAULT_TITLE_PREFIXES: [&str; 2] = ["New session - ", "Child session - "];

/// The address in the server's listening line, any IP; `None` for any other line.
pub fn listening_addr(line: &str) -> Option<SocketAddr> {
    let rest = line.trim().strip_prefix(LISTENING_PREFIX)?;

    rest.trim_end_matches('/').parse().ok()
}

/// One event of `GET /global/event`: `{directory, payload: {type, properties}}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope {
    /// The event's type (`payload.type`).
    pub kind: String,
    /// `payload.properties`.
    pub properties: Value,
}

impl Envelope {
    /// The session the event is about (`properties.sessionID`).
    pub fn session_id(&self) -> Option<&str> {
        self.properties.get("sessionID").and_then(Value::as_str)
    }
}

/// Reads an event's `data:`; `None` when it is not an event object.
pub fn envelope(data: &str) -> Option<Envelope> {
    let value: Value = serde_json::from_str(data).ok()?;
    let payload = value.get("payload")?;

    Some(Envelope {
        kind: payload.get("type")?.as_str()?.to_owned(),
        properties: payload.get("properties").cloned().unwrap_or(Value::Null),
    })
}

/// A model by provider and id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRef {
    /// Provider id (`providerID`).
    pub provider: String,
    /// Model id within the provider (`modelID`).
    pub id: String,
}

/// The fields of a session (`Session.Info`) the chat uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    /// `ses_…`.
    pub id: String,
    /// The working directory, as OpenCode stored it.
    pub directory: String,
    /// The session this one was started from (subagent sessions).
    pub parent_id: Option<String>,
    /// The title, unless OpenCode has not named the session yet.
    pub title: Option<String>,
    /// The agent of the last prompt.
    pub agent: Option<String>,
    /// The model of the last prompt.
    pub model: Option<ModelRef>,
    /// The model variant of the last prompt (`default` is none).
    pub variant: Option<String>,
}

/// Reads a session; `None` without an id or a directory.
pub fn session_info(value: &Value) -> Option<SessionInfo> {
    let model = value.get("model");

    Some(SessionInfo {
        id: string(value, "id")?,
        directory: string(value, "directory")?,
        parent_id: string(value, "parentID"),
        title: string(value, "title").filter(|title| !is_default_title(title)),
        agent: string(value, "agent"),
        model: model.and_then(|model| {
            Some(ModelRef {
                provider: string(model, "providerID")?,
                id: string(model, "id")?,
            })
        }),
        variant: model
            .and_then(|model| string(model, "variant"))
            .filter(|variant| variant != "default"),
    })
}

/// True for the title OpenCode gives a session it has not named yet (`New session - <ISO time>`).
pub fn is_default_title(title: &str) -> bool {
    DEFAULT_TITLE_PREFIXES.iter().any(|prefix| {
        title
            .strip_prefix(prefix)
            .is_some_and(|time| time.len() == 24 && time.ends_with('Z') && time.as_bytes()[10] == b'T')
    })
}

/// A model of `GET /config/providers` with its variants (the app's thinking levels).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderModel {
    /// The display fields.
    pub option: ModelOption,
    /// Variant names, in the server's order.
    pub variants: Vec<String>,
}

impl ProviderModel {
    /// True for the model `model` names.
    pub fn is(&self, model: &ModelRef) -> bool {
        self.option.provider == model.provider && self.option.id == model.id
    }
}

/// The models of every provider in `GET /config/providers` (`{providers, default}`).
pub fn provider_models(body: &Value) -> Vec<ProviderModel> {
    let providers = body.get("providers").and_then(Value::as_array).into_iter().flatten();
    let mut models = Vec::new();

    for provider in providers {
        let Some(provider_id) = string(provider, "id") else {
            continue;
        };
        let entries = provider.get("models").and_then(Value::as_object).into_iter().flatten();

        for (model_id, model) in entries {
            models.push(provider_model(&provider_id, model_id, model));
        }
    }

    models
}

fn provider_model(provider: &str, id: &str, model: &Value) -> ProviderModel {
    let capabilities = model.get("capabilities").unwrap_or(&Value::Null);
    let input = capabilities
        .get("input")
        .and_then(Value::as_object)
        .map(enabled_keys)
        .unwrap_or_default();

    let option = ModelOption {
        provider: provider.to_owned(),
        id: id.to_owned(),
        name: string(model, "name"),
        reasoning: capabilities.get("reasoning").and_then(Value::as_bool).unwrap_or(false),
        context_window: model.pointer("/limit/context").and_then(Value::as_u64),
        max_tokens: model.pointer("/limit/output").and_then(Value::as_u64),
        input,
    };

    let variants = model
        .get("variants")
        .and_then(Value::as_object)
        .map(|variants| variants.keys().cloned().collect())
        .unwrap_or_default();

    ProviderModel { option, variants }
}

fn enabled_keys(kinds: &Map<String, Value>) -> Vec<String> {
    kinds
        .iter()
        .filter(|(_, enabled)| enabled.as_bool() == Some(true))
        .map(|(kind, _)| kind.clone())
        .collect()
}

/// The agents of `GET /agent` the chat can switch to: primary ones (`primary` or `all`) that are
/// not hidden, in the server's order, which puts the default agent first.
pub fn primary_agents(body: &Value) -> Vec<AgentOption> {
    let agents = body.as_array().into_iter().flatten();

    agents
        .filter(|agent| string(agent, "mode").as_deref() != Some("subagent"))
        .filter(|agent| agent.get("hidden").and_then(Value::as_bool) != Some(true))
        .filter_map(|agent| {
            let id = string(agent, "name")?;

            Some(AgentOption {
                name: id.clone(),
                id,
                description: string(agent, "description"),
            })
        })
        .collect()
}

/// The body of `POST /session/{id}/prompt_async`: the text, each image as a `data:` URL file part,
/// and the agent, model and variant to use. Never `tools` (it would replace the session's
/// permission rules).
pub fn prompt_body(
    message: &str,
    images: &[ImageAttachment],
    agent: &str,
    model: Option<&ModelRef>,
    variant: Option<&str>,
) -> Value {
    let mut parts = vec![json!({ "type": "text", "text": message })];

    for (index, image) in images.iter().enumerate() {
        parts.push(json!({
            "type": "file",
            "mime": image.mime_type,
            "filename": image_name(index, &image.mime_type),
            "url": format!("data:{};base64,{}", image.mime_type, image.data),
        }));
    }

    let mut body = json!({ "parts": parts, "agent": agent });

    if let Some(model) = model {
        body["model"] = json!({ "providerID": model.provider, "modelID": model.id });
    }

    if let Some(variant) = variant {
        body["variant"] = json!(variant);
    }

    body
}

/// `image-<n>.<ext>` for the `n`th (1-based) image.
fn image_name(index: usize, mime_type: &str) -> String {
    let extension = mime_type.strip_prefix("image/").unwrap_or("bin");

    format!("image-{}.{extension}", index + 1)
}

fn string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}
