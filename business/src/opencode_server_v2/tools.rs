//! OpenCode 2 tool ids (`shell`, `patch`, `subagent`, ...) and the project files their calls may
//! have changed, tool calls as OpenCode 2 stores them ([`ToolCall`]), and how a call's output
//! shows ([`content_text`], [`failure_text`]). Shared by the CLI transcript and the live chat, so a
//! card shows the same output live and after resume.
//!
//! `edit` and `write` name their file in `path`; `patch` names its files in the `*** Add File:`,
//! `*** Update File:`, `*** Delete File:` and `*** Move to:` headers of `patchText`
//! (`packages/util/src/patch.ts`, headers matched after trimming the line). OpenCode 2 migrates
//! OpenCode 1 sessions with their tool calls unchanged, so the OpenCode 1 names (`apply_patch`,
//! `multiedit`) and the `filePath` argument count too. A relative path resolves against the
//! session's directory, as OpenCode resolves it; only paths inside the project count
//! ([`touched_paths`](crate::pi_family::touched::touched_paths)).

use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

pub use super::events::ToolContent;
use super::messages::{SessionError, Times};
use crate::model::ChatBlock;
use crate::pi_family::touched::touched_paths;
use crate::security::redact_text;

/// Runs a shell command (OpenCode 1: `bash`).
pub const SHELL: &str = "shell";
/// Reads a file.
pub const READ: &str = "read";
/// Finds files by pattern.
pub const GLOB: &str = "glob";
/// Searches file contents.
pub const GREP: &str = "grep";
/// Replaces text in the file in `path`.
pub const EDIT: &str = "edit";
/// Writes the file in `path`.
pub const WRITE: &str = "write";
/// Applies a patch (OpenCode 1: `apply_patch`).
pub const PATCH: &str = "patch";
/// Runs a subagent (OpenCode 1: `task`).
pub const SUBAGENT: &str = "subagent";
/// Asks the user questions.
pub const QUESTION: &str = "question";
/// Updates the todo list.
pub const TODO_WRITE: &str = "todowrite";
/// Loads a skill.
pub const SKILL: &str = "skill";
/// Fetches a URL.
pub const WEBFETCH: &str = "webfetch";
/// Searches the web.
pub const WEBSEARCH: &str = "websearch";

/// Tools whose calls change files: the OpenCode 2 ids and the OpenCode 1 ids a migrated session
/// may still hold.
pub const FILE_CHANGE_TOOLS: [&str; 5] = [EDIT, WRITE, PATCH, "apply_patch", "multiedit"];

/// Arguments that name the changed file (`filePath` in migrated OpenCode 1 calls).
const PATH_ARGS: [&str; 2] = ["path", "filePath"];

/// The argument holding a patch.
const PATCH_ARG: &str = "patchText";

/// Patch headers followed by a file path.
const PATCH_HEADERS: [&str; 4] = ["*** Add File:", "*** Update File:", "*** Delete File:", "*** Move to:"];

/// True when calls of `tool` change files (ASCII case-insensitive).
pub fn is_file_change(tool: &str) -> bool {
    FILE_CHANGE_TOOLS.iter().any(|name| name.eq_ignore_ascii_case(tool))
}

/// The paths, as written, a call of `tool` with `input` names as files it changes.
pub fn named_paths<'a>(tool: &str, input: &'a Value) -> Vec<&'a str> {
    if !is_file_change(tool) {
        return Vec::new();
    }

    let mut paths: Vec<&str> = PATH_ARGS
        .iter()
        .filter_map(|key| input.get(key).and_then(Value::as_str))
        .collect();

    if let Some(patch) = input.get(PATCH_ARG).and_then(Value::as_str) {
        paths.extend(patch_paths(patch));
    }

    paths
}

/// The project-relative files (forward slashes) a call of `tool` with `input`, in a session working
/// in `directory`, may have changed; each file once, in the order the call names them.
pub fn call_touched(project: &Path, directory: &Path, tool: &str, input: &Value) -> Vec<String> {
    let mut touched: Vec<String> = Vec::new();

    for raw in named_paths(tool, input) {
        let absolute = directory.join(raw);
        let args = json!({ "path": absolute.to_string_lossy() });

        for path in touched_paths(project, EDIT, &args) {
            if !touched.contains(&path) {
                touched.push(path);
            }
        }
    }

    touched
}

/// The file paths named by the headers of a patch.
pub fn patch_paths(patch: &str) -> impl Iterator<Item = &str> {
    patch.lines().filter_map(|line| {
        let line = line.trim();

        PATCH_HEADERS
            .iter()
            .find_map(|header| line.strip_prefix(header))
            .map(str::trim)
            .filter(|path| !path.is_empty())
    })
}

/// A tool call and its state.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub state: ToolState,
    pub time: Times,
}

impl Default for ToolCall {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            state: ToolState::Other,
            time: Times::default(),
        }
    }
}

/// Where a tool call is. While `streaming` the input is still partial JSON text and not kept.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum ToolState {
    Streaming,
    Running {
        #[serde(default)]
        input: Value,
    },
    Completed {
        #[serde(default)]
        input: Value,
        #[serde(default)]
        content: Vec<ToolContent>,
    },
    Error {
        #[serde(default)]
        input: Value,
        #[serde(default)]
        error: SessionError,
        #[serde(default)]
        content: Vec<ToolContent>,
    },
    #[serde(other)]
    Other,
}

impl ToolCall {
    /// The call's arguments; `Null` while they stream.
    pub fn input(&self) -> &Value {
        match &self.state {
            ToolState::Running { input } | ToolState::Completed { input, .. } | ToolState::Error { input, .. } => input,
            ToolState::Streaming | ToolState::Other => &Value::Null,
        }
    }

    /// The redacted output of a finished call and whether it failed; `None` while it runs.
    pub fn outcome(&self) -> Option<(String, bool)> {
        let (output, is_error) = match &self.state {
            ToolState::Completed { content, .. } => (content_text(content), false),
            ToolState::Error { error, content, .. } => (failure_text(&error.message, content, None), true),
            _ => return None,
        };

        Some((redact_text(&output), is_error))
    }

    /// The project-relative files this call, in a session working in `directory`, may have
    /// changed (see the module docs).
    pub fn touched(&self, project: &Path, directory: &Path) -> Vec<String> {
        call_touched(project, directory, &self.name, self.input())
    }

    /// The call as a chat block; arguments that are not an object become `{}`.
    pub fn block(&self) -> ChatBlock {
        let input = self.input();
        let args = if input.is_object() { input.clone() } else { json!({}) };

        ChatBlock::ToolCall {
            id: self.id.clone(),
            name: self.name.clone(),
            args,
            intent: None,
        }
    }
}

/// A tool's output as its card shows it, live and after resume: the text items one per line, a
/// named file (an image a tool read) as `@name`; unnamed files are not shown.
pub fn content_text(content: &[ToolContent]) -> String {
    let lines: Vec<String> = content
        .iter()
        .filter_map(|item| match item {
            ToolContent::Text { text } => Some(text.clone()),
            ToolContent::File { name: Some(name), .. } => Some(format!("@{name}")),
            _ => None,
        })
        .collect();

    lines.join("\n")
}

/// A failed call's output as its card shows it: what the call wrote before it failed (`content`,
/// else the output streamed while it ran), then the `error`.
pub fn failure_text(error: &str, content: &[ToolContent], streamed: Option<&str>) -> String {
    let written = content_text(content);
    let partial = match streamed {
        Some(streamed) if written.is_empty() => streamed,
        _ => written.as_str(),
    };

    let lines: Vec<&str> = [partial.trim_end(), error]
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect();

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{named_paths, patch_paths};

    #[test]
    fn patch_headers_name_their_files() {
        let patch = "*** Begin Patch\n*** Add File: a.txt\n+hi\n  *** Update File: src/b.rs\n*** Move to: src/c.rs\n@@\n-x\n+y\n*** Delete File: old.txt\n*** Add File:   \n*** End Patch";

        assert_eq!(
            patch_paths(patch).collect::<Vec<_>>(),
            ["a.txt", "src/b.rs", "src/c.rs", "old.txt"]
        );
    }

    #[test]
    fn only_file_changing_tools_name_paths() {
        let input = json!({ "path": "a.txt", "filePath": "b.txt", "patchText": "*** Add File: c.txt" });

        assert_eq!(named_paths("edit", &input), ["a.txt", "b.txt", "c.txt"]);
        assert_eq!(named_paths("Apply_Patch", &input), ["a.txt", "b.txt", "c.txt"]);
        assert!(named_paths("read", &input).is_empty());
        assert!(named_paths("shell", &json!({ "command": "rm a.txt" })).is_empty());
    }
}
