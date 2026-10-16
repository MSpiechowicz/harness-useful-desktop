//! OpenCode agents (`agent/agent.ts`, `config/agent.ts`): the built-ins, the `agent` key of each
//! settings file and the Markdown files in `agent/` and `agents/` of each config directory
//! (named by their path below it). Later definitions win field by field. The legacy `mode` key
//! and `mode/` / `modes/` files define primary agents, and `disable: true` removes an agent,
//! built-in ones included.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::super::config::{Place, Step};
use super::{description, markdown_fields, markdown_files, object_entries};
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

/// See the module docs. Built-in agents come first, in OpenCode's order, then the others by name.
pub(super) fn agents(steps: &[Step]) -> Vec<AgentInfo> {
    let definitions = definitions(steps);

    let mut agents: Vec<AgentInfo> = BUILTINS.iter().map(builtin).collect();
    let mut custom: Vec<AgentInfo> = Vec::new();

    for definition in definitions {
        if definition.fields.get("disable").and_then(Value::as_bool) == Some(true) {
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

/// Every agent definition in merge order; `mode` key entries are applied last, as OpenCode does.
fn definitions(steps: &[Step]) -> Vec<Definition> {
    let mut definitions: Vec<Definition> = Vec::new();
    let mut modes: Vec<Definition> = Vec::new();

    for step in steps {
        match step {
            Step::File { path, place, document } => {
                for (name, fields) in object_entries(document.get("agent")) {
                    if let Some(fields) = fields.as_object() {
                        define(&mut definitions, name, fields.clone(), path, *place);
                    }
                }

                for (name, fields) in object_entries(document.get("mode")) {
                    if let Some(fields) = fields.as_object() {
                        define(&mut modes, name, primary(fields.clone()), path, *place);
                    }
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

    if let Some(model) = text_field(&fields, "model") {
        agent.model = Some(redact_text(&model));
    }

    if let Some(hidden) = fields.get("hidden").and_then(Value::as_bool) {
        agent.hidden = hidden;
    }
}

fn parse_mode(value: &str) -> Option<AgentMode> {
    match value {
        "primary" => Some(AgentMode::Primary),
        "subagent" => Some(AgentMode::Subagent),
        "all" => Some(AgentMode::All),
        _ => None,
    }
}
