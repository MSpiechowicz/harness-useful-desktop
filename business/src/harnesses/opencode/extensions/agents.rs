//! OpenCode agents (`agent/agent.ts`, `config/agent.ts`): the built-ins, the `agent` key of each
//! settings file and the Markdown files in `agent/` and `agents/` of each config directory
//! (named by their path below it). Later definitions win field by field. The legacy `mode` key
//! and `mode/` / `modes/` files define primary agents, and `disable: true` removes an agent,
//! built-in ones included.
//!
//! OpenCode 2 (`core/src/config/plugin/agent.ts`, `core/src/config/normalize.ts`, 2.1.2) also reads
//! the `agents` key: in one file, a `mode` entry replaces an `agent` entry of the same name and an
//! `agents` entry replaces both. It removes an agent with `disabled: true` (the legacy `disable`
//! still works), and a model may be `{providerID, model}`.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::super::config::{Layers, Place, Step, object_entries, replace};
use super::{description, markdown_fields, markdown_files};
use crate::harness::OpenCodeGeneration;
use crate::model::{AgentInfo, AgentMode, ExtensionScope};
use crate::pi_family::text_field;
use crate::security::redact_text;

/// One built-in agent: name, description, mode and whether OpenCode hides it from pickers.
type Builtin = (&'static str, Option<&'static str>, AgentMode, bool);

/// OpenCode's built-in agents; the last three are internal helpers it hides.
const BUILTINS: &[Builtin] = &[
    (
        "build",
        Some("The default agent. Executes tools based on configured permissions."),
        AgentMode::Primary,
        false,
    ),
    (
        "plan",
        Some("Plan mode. Disallows all edit tools."),
        AgentMode::Primary,
        false,
    ),
    (
        "general",
        Some("General-purpose agent for researching complex questions and executing multi-step tasks."),
        AgentMode::Subagent,
        false,
    ),
    (
        "explore",
        Some("Fast agent specialized for exploring codebases."),
        AgentMode::Subagent,
        false,
    ),
    ("compaction", None, AgentMode::Primary, true),
    ("title", None, AgentMode::Primary, true),
    ("summary", None, AgentMode::Primary, true),
];

/// The merged settings of one agent and the file that set them last.
struct Definition {
    name: String,
    fields: Map<String, Value>,
    path: PathBuf,
    place: Place,
}

/// The OpenCode 2 description of its `plan` agent.
const V2_PLAN: &str = "Read-only agent for exploring the codebase and planning work before implementation.";

/// See the module docs. Built-in agents come first, in OpenCode's order, then the others by name.
pub(super) fn agents(layers: &Layers) -> Vec<AgentInfo> {
    let (definitions, removal) = match layers.generation {
        OpenCodeGeneration::V1 => (definitions(&layers.steps), "disable"),
        OpenCodeGeneration::V2 => (v2_definitions(&layers.steps), "disabled"),
    };

    let mut agents: Vec<AgentInfo> = BUILTINS.iter().map(builtin).collect();
    if layers.generation == OpenCodeGeneration::V2
        && let Some(plan) = agents.iter_mut().find(|agent| agent.name == "plan")
    {
        plan.description = Some(V2_PLAN.to_owned());
    }

    let mut custom: Vec<AgentInfo> = Vec::new();

    for definition in definitions {
        if definition.fields.get(removal).and_then(Value::as_bool) == Some(true) {
            agents.retain(|agent| agent.name != definition.name);
            continue;
        }

        if let Some(agent) = agents.iter_mut().find(|agent| agent.name == definition.name) {
            apply(agent, &definition.fields);
            continue;
        }

        let mut agent = AgentInfo {
            name: definition.name,
            description: None,
            mode: AgentMode::All,
            model: None,
            built_in: false,
            hidden: false,
            scope: definition.place.scope(),
            path: Some(definition.path.to_string_lossy().into_owned()),
        };
        apply(&mut agent, &definition.fields);
        custom.push(agent);
    }

    custom.sort_by(|left, right| left.name.cmp(&right.name));
    agents.extend(custom);
    agents
}

/// Every OpenCode 1 agent definition in merge order; `mode` key entries are applied last, as
/// OpenCode 1 does.
fn definitions(steps: &[Step]) -> Vec<Definition> {
    let mut definitions: Vec<Definition> = Vec::new();
    let mut modes: Vec<Definition> = Vec::new();

    for step in steps {
        match step {
            Step::File { path, place, document } => {
                let document = document.value().unwrap_or(&Value::Null);

                for (name, fields) in objects(document, "agent") {
                    define(&mut definitions, &name, fields, path, *place);
                }

                for (name, fields) in objects(document, "mode") {
                    define(&mut modes, &name, primary(fields), path, *place);
                }
            }

            Step::Dir(dir) => {
                for (name, file) in markdown_files(&dir.path, &["agent", "agents"], true) {
                    if let Some(fields) = markdown_fields(&file) {
                        define(&mut definitions, &name, fields, &file, dir.place);
                    }
                }

                for (name, file) in markdown_files(&dir.path, &["mode", "modes"], false) {
                    if let Some(fields) = markdown_fields(&file) {
                        define(&mut definitions, &name, primary(fields), &file, dir.place);
                    }
                }
            }
        }
    }

    for mode in modes {
        define(&mut definitions, &mode.name, mode.fields, &mode.path, mode.place);
    }

    definitions
}

/// Every OpenCode 2 agent definition in merge order, with legacy `disable` read as `disabled`.
fn v2_definitions(steps: &[Step]) -> Vec<Definition> {
    let mut definitions: Vec<Definition> = Vec::new();

    for step in steps {
        match step {
            Step::File { path, place, document } => {
                let document = document.value().unwrap_or(&Value::Null);

                for (name, fields) in v2_file_agents(document) {
                    define(&mut definitions, &name, fields, path, *place);
                }
            }

            Step::Dir(dir) => {
                for (name, file) in markdown_files(&dir.path, &["agent", "agents"], true) {
                    if let Some(fields) = markdown_fields(&file) {
                        define(&mut definitions, &name, legacy(fields), &file, dir.place);
                    }
                }

                for (name, file) in markdown_files(&dir.path, &["mode", "modes"], false) {
                    if let Some(fields) = markdown_fields(&file) {
                        define(&mut definitions, &name, primary(legacy(fields)), &file, dir.place);
                    }
                }
            }
        }
    }

    definitions
}

/// The agents of one OpenCode 2 settings file: `agent`, then `mode` (primary), then `agents`, each
/// replacing a whole entry of the same name.
fn v2_file_agents(document: &Value) -> Vec<(String, Map<String, Value>)> {
    let mut agents = Vec::new();

    for (name, fields) in objects(document, "agent") {
        replace(&mut agents, &name, legacy(fields));
    }

    for (name, fields) in objects(document, "mode") {
        replace(&mut agents, &name, primary(legacy(fields)));
    }

    for (name, fields) in objects(document, "agents") {
        replace(&mut agents, &name, fields);
    }

    agents
}

/// The object entries of `key` whose values are objects.
fn objects(document: &Value, key: &str) -> Vec<(String, Map<String, Value>)> {
    object_entries(document, key)
        .into_iter()
        .filter_map(|(name, fields)| match fields {
            Value::Object(fields) => Some((name, fields)),
            _ => None,
        })
        .collect()
}

/// An OpenCode 1 definition as OpenCode 2 reads it: `disable` becomes `disabled`.
fn legacy(mut fields: Map<String, Value>) -> Map<String, Value> {
    if let Some(disable) = fields.remove("disable") {
        fields.entry("disabled").or_insert(disable);
    }

    fields
}

/// Adds `fields` to the definition of `name` (later fields win) and records where they came from.
fn define(definitions: &mut Vec<Definition>, name: &str, fields: Map<String, Value>, path: &Path, place: Place) {
    match definitions.iter_mut().find(|definition| definition.name == name) {
        Some(definition) => {
            definition.fields.extend(fields);
            definition.path = path.to_path_buf();
            definition.place = place;
        }
        None => definitions.push(Definition {
            name: name.to_owned(),
            fields,
            path: path.to_path_buf(),
            place,
        }),
    }
}

fn primary(mut fields: Map<String, Value>) -> Map<String, Value> {
    fields.insert("mode".to_owned(), Value::from("primary"));
    fields
}

fn builtin(&(name, description, mode, hidden): &Builtin) -> AgentInfo {
    AgentInfo {
        name: name.to_owned(),
        description: description.map(str::to_owned),
        mode,
        model: None,
        built_in: true,
        hidden,
        scope: ExtensionScope::Builtin,
        path: None,
    }
}

/// Applies the fields the view shows; unknown modes and wrong types are ignored.
fn apply(agent: &mut AgentInfo, fields: &Map<String, Value>) {
    let fields = Value::Object(fields.clone());

    if let Some(description) = description(&fields) {
        agent.description = Some(description);
    }

    if let Some(mode) = fields.get("mode").and_then(Value::as_str).and_then(parse_mode) {
        agent.mode = mode;
    }

    if let Some(model) = model(&fields) {
        agent.model = Some(redact_text(&model));
    }

    if let Some(hidden) = fields.get("hidden").and_then(Value::as_bool) {
        agent.hidden = hidden;
    }
}

/// `provider/model` as written, or built from OpenCode 2's `{providerID, model}`.
fn model(fields: &Value) -> Option<String> {
    if let Some(model) = text_field(fields, "model") {
        return Some(model);
    }

    let selection = fields.get("model")?;
    let provider = selection.get("providerID")?.as_str()?;
    let model = selection.get("model")?.as_str()?;

    Some(format!("{provider}/{model}"))
}

fn parse_mode(value: &str) -> Option<AgentMode> {
    match value {
        "primary" => Some(AgentMode::Primary),
        "subagent" => Some(AgentMode::Subagent),
        "all" => Some(AgentMode::All),
        _ => None,
    }
}
