//! The chat's own "Always allow" answers, for asks that bring no rule to echo back (every ask the
//! Always ask hook makes, and asks without `permission_suggestions`). Nothing is written anywhere:
//! the list lasts as long as the chat's process, and later asks it covers are allowed without a
//! dialog.
//!
//! - **File tools** (`Read`, `Write`, `Edit`, `MultiEdit`, `NotebookEdit`) on a file inside the
//!   project are kept by the tool: a later call of that tool is covered when its file lies inside
//!   the project too (after canonicalization, as for
//!   [`touched_paths`](crate::pi_family::touched::touched_paths)).
//! - **Every other call** is kept by its tool and exact input, so only the same call again is
//!   covered. For `Bash` that is the exact command text with every other input field equal
//!   (`timeout`, `run_in_background`, `dangerouslyDisableSandbox`, …), only the model's
//!   `description` of the command may differ. There are no command prefixes: those come only from
//!   the CLI's own suggestions.
//! - An ask the CLI suppresses lasting rules for, blocked on a path or flagged by a safety check
//!   ([`PermissionAsk::may_always_allow`]) is never covered.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::claude_stream::mapper::PermissionAsk;
use crate::claude_stream::tools::{BASH, EDIT, MULTI_EDIT, NOTEBOOK_EDIT, READ, WRITE};
use crate::pi_family::touched::touched_paths;
use crate::security::redact_text;

/// `Bash` input fields that only describe the command to the user.
const BASH_DISPLAY_FIELDS: [&str; 1] = ["description"];

/// What one "Always allow" answer allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowKey {
    tool: String,
    scope: Scope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Scope {
    /// Calls on files inside the project.
    ProjectFiles,
    /// Calls with exactly this input (for `Bash` without its display fields).
    Exact(Value),
}

impl AllowKey {
    /// The key an "Always allow" answer to `ask` keeps, in the chat of `project`.
    pub fn of(ask: &PermissionAsk, project: &Path) -> Self {
        let scope = if in_project(ask, project) {
            Scope::ProjectFiles
        } else {
            Scope::Exact(exact_input(ask))
        };

        Self {
            tool: ask.tool.clone(),
            scope,
        }
    }

    /// True when the key allows `ask`, in the chat of `project`.
    pub fn covers(&self, ask: &PermissionAsk, project: &Path) -> bool {
        if ask.tool != self.tool || !ask.may_always_allow() {
            return false;
        }

        match &self.scope {
            Scope::ProjectFiles => in_project(ask, project),
            Scope::Exact(input) => *input == exact_input(ask),
        }
    }

    /// What the key allows, for the dialog (`label` names the tool).
    pub fn describe(&self, label: &str) -> String {
        let allowed = match &self.scope {
            Scope::ProjectFiles => format!("Allows every {label} call on files in this project"),
            Scope::Exact(_) if self.tool == BASH => "Allows this exact command again".to_owned(),
            Scope::Exact(_) => format!("Allows this exact {label} call again"),
        };

        redact_text(&format!("{allowed} until this chat ends"))
    }
}

/// The chat's kept "Always allow" answers.
#[derive(Debug)]
pub struct AllowList {
    project: PathBuf,
    keys: Vec<AllowKey>,
}

impl AllowList {
    /// An empty list for the chat of `project`.
    pub fn new(project: &Path) -> Self {
        Self {
            project: project.to_path_buf(),
            keys: Vec::new(),
        }
    }

    /// Keeps `key`.
    pub fn remember(&mut self, key: AllowKey) {
        if !self.keys.contains(&key) {
            self.keys.push(key);
        }
    }

    /// True when a kept answer allows `ask`.
    pub fn covers(&self, ask: &PermissionAsk) -> bool {
        self.keys.iter().any(|key| key.covers(ask, &self.project))
    }
}

/// The file a file tool's call names; `None` for other tools.
fn file_path(ask: &PermissionAsk) -> Option<&str> {
    let field = match ask.tool.as_str() {
        READ | WRITE | EDIT | MULTI_EDIT => "file_path",
        NOTEBOOK_EDIT => "notebook_path",
        _ => return None,
    };

    ask.input.get(field).and_then(Value::as_str)
}

/// True for a file tool's call on a file inside `project`.
fn in_project(ask: &PermissionAsk, project: &Path) -> bool {
    let Some(path) = file_path(ask) else {
        return false;
    };

    !touched_paths(project, "write", &json!({ "file_path": path })).is_empty()
}

/// The input an exact key compares (see the module docs).
fn exact_input(ask: &PermissionAsk) -> Value {
    let mut input = ask.input.clone();

    if ask.tool == BASH
        && let Some(fields) = input.as_object_mut()
    {
        for field in BASH_DISPLAY_FIELDS {
            fields.remove(field);
        }
    }

    input
}
