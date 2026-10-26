//! Claude Code tool ids (`Bash`, `Edit`, `Write`, `Task`, ...) and the project files their calls
//! may have changed. Shared by the session reader and the live chat, so the Sessions view and a
//! hook's `toolEnd` agree on what a call touched.
//!
//! `Edit`, `MultiEdit` and `Write` name their file in `file_path`, `NotebookEdit` in
//! `notebook_path`. Only paths inside the project count
//! ([`touched_paths`](crate::pi_family::touched::touched_paths)).

use std::path::Path;

use serde_json::{Value, json};

use crate::pi_family::touched::touched_paths as contained_paths;

/// Runs a shell command.
pub const BASH: &str = "Bash";
/// Reads a file.
pub const READ: &str = "Read";
/// Replaces text in the file in `file_path`.
pub const EDIT: &str = "Edit";
/// Several replacements in the file in `file_path` (older Claude Code versions).
pub const MULTI_EDIT: &str = "MultiEdit";
/// Writes the file in `file_path`.
pub const WRITE: &str = "Write";
/// Edits a cell of the notebook in `notebook_path`.
pub const NOTEBOOK_EDIT: &str = "NotebookEdit";
/// Runs a subagent (Claude Code names it `Task`, newer versions also `Agent`).
pub const TASK: &str = "Task";
/// Asks the user questions, answered through the permission prompt.
pub const ASK_USER_QUESTION: &str = "AskUserQuestion";

/// Tools whose calls change files.
pub const FILE_CHANGE_TOOLS: [&str; 4] = [EDIT, MULTI_EDIT, WRITE, NOTEBOOK_EDIT];

/// True when calls of `tool` change files (exact Claude Code tool id).
pub fn is_file_change(tool: &str) -> bool {
    FILE_CHANGE_TOOLS.contains(&tool)
}

/// The argument naming the file a call of `tool` changes.
fn path_arg(tool: &str) -> Option<&'static str> {
    match tool {
        EDIT | MULTI_EDIT | WRITE => Some("file_path"),
        NOTEBOOK_EDIT => Some("notebook_path"),
        _ => None,
    }
}

/// The project-relative paths a call of `tool` with `input` may have changed, inside `project`
/// only, with forward slashes.
pub fn touched_paths(project: &Path, tool: &str, input: &Value) -> Vec<String> {
    let Some(arg) = path_arg(tool) else {
        return Vec::new();
    };

    let Some(path) = input.get(arg).and_then(Value::as_str) else {
        return Vec::new();
    };

    contained_paths(project, "write", &json!({ "file_path": path }))
}
