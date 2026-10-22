//! Claude Code agents and commands, both Markdown files with YAML frontmatter, from three places:
//! `.claude/agents` / `.claude/commands` from the project up to its repository root, the config
//! directory's `agents/` / `commands/`, and the same folders of each enabled plugin (named
//! `<plugin>:<name>`). Only the frontmatter is read, never the prompt below it.
//!
//! - Agents need a `name` and a `description` (Claude skips other files). They are subagents with
//!   an optional `model` (`sonnet`, `opus`, `inherit`, a model id). A project agent outranks a user
//!   agent of the same name.
//! - Commands are named after their file and may set `description` and `argument-hint`. A user
//!   command outranks a project one, as for skills.
//!
//! Only the `*.md` files directly in each folder are listed. Each layer's scans share a
//! [`ScanBudget`], and a plugin's files must resolve inside the plugin.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::plugins::EnabledPlugin;
use crate::model::{AgentInfo, AgentMode, ExtensionScope, PromptTemplateInfo};
use crate::pi_family::{PerLayer, ScanBudget, text_field};
use crate::security::redact_text;

/// One folder to list, with the scope and namespace of what it holds.
pub(super) struct Folder {
    pub dir: PathBuf,
    pub scope: ExtensionScope,
    /// The plugin directory the files must stay in, and the plugin name prefixed to theirs.
    pub plugin: Option<(PathBuf, String)>,
}

impl Folder {
    pub(super) fn plain(dir: PathBuf, scope: ExtensionScope) -> Self {
        Self {
            dir,
            scope,
            plugin: None,
        }
    }

    pub(super) fn of_plugin(plugin: &EnabledPlugin, sub: &str) -> Self {
        Self {
            dir: plugin.dir.join(sub),
            scope: ExtensionScope::Plugin,
            plugin: Some((plugin.dir.clone(), plugin.name.clone())),
        }
    }

    fn named(&self, name: String) -> String {
        match &self.plugin {
            Some((_, plugin)) => format!("{plugin}:{name}"),
            None => name,
        }
    }
}

/// Agents from `folders`, in precedence order (the first of each name wins).
pub(super) fn agents(folders: &[Folder], budgets: &mut PerLayer<ScanBudget>) -> Vec<AgentInfo> {
    let mut agents: Vec<AgentInfo> = Vec::new();

    for folder in folders {
        for (path, frontmatter) in markdown(folder, budgets) {
            let (Some(name), Some(description)) = (
                text_field(&frontmatter, "name"),
                text_field(&frontmatter, "description"),
            ) else {
                continue;
            };

            let name = folder.named(name);
            if agents.iter().any(|agent| agent.name == name) {
                continue;
            }

            agents.push(AgentInfo {
                name,
                description: Some(redact_text(&description)),
                mode: AgentMode::Subagent,
                model: text_field(&frontmatter, "model").map(|model| redact_text(&model)),
                built_in: false,
                hidden: false,
                scope: folder.scope,
                path: Some(path.to_string_lossy().into_owned()),
            });
        }
    }

    agents
}

/// Commands from `folders`, in precedence order (the first of each name wins).
pub(super) fn commands(folders: &[Folder], budgets: &mut PerLayer<ScanBudget>) -> Vec<PromptTemplateInfo> {
    let mut commands: Vec<PromptTemplateInfo> = Vec::new();

    for folder in folders {
        for (path, frontmatter) in markdown(folder, budgets) {
            let stem = path.file_stem().map(|stem| stem.to_string_lossy().into_owned());
            let Some(name) = stem.map(|stem| folder.named(stem)) else {
                continue;
            };

            if commands.iter().any(|command| command.name == name) {
                continue;
            }

            commands.push(PromptTemplateInfo {
                name,
                description: text_field(&frontmatter, "description").map(|text| redact_text(&text)),
                argument_hint: text_field(&frontmatter, "argument-hint").map(|text| redact_text(&text)),
                scope: folder.scope,
                path: path.to_string_lossy().into_owned(),
            });
        }
    }

    commands
}

/// The `*.md` files directly in `folder` (inside its plugin, if any) with their frontmatter
/// (`null` without one), charged to the folder's layer.
fn markdown(folder: &Folder, budgets: &mut PerLayer<ScanBudget>) -> Vec<(PathBuf, Value)> {
    let budget = budgets.get(folder.scope);
    let boundary: Option<&Path> = folder.plugin.as_ref().map(|(dir, _)| dir.as_path());

    let files: Vec<PathBuf> = budget
        .list_dir(&folder.dir)
        .into_iter()
        .filter(|entry| entry.is_file && entry.name.ends_with(".md") && !entry.name.starts_with('.'))
        .map(|entry| entry.path)
        .collect();

    let mut read = Vec::new();

    for path in files {
        if boundary.is_some_and(|boundary| !budget.is_inside(boundary, &path)) {
            continue;
        }

        let frontmatter = budget.read_frontmatter(&path).unwrap_or(Value::Null);
        read.push((path, frontmatter));
    }

    read
}
