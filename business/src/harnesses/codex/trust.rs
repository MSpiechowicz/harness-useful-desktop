//! Project trust of a Codex chat (plan Decision 2, PI's trust-once pattern).
//!
//! Codex applies a project's own layer (`<project>/.codex/config.toml`, its hooks and rules) only
//! in a folder the user's `config.toml` trusts (`[projects."<path>"] trust_level = "trusted"`),
//! and ignores it elsewhere: a chat starts either way, so nothing is refused here
//! (`capabilities.projectTrustRequired` stays off).
//!
//! - **Trust once:** the launch passes `-c projects={"<project>"={trust_level="trusted"}}` for
//!   that process only ([`trust_once_args`]); `config.toml` is never written. The project is a
//!   quoted key inside an inline table, not a dotted `-c` key path, so a path with dots or
//!   quotes stays one key. A path that is not absolute UTF-8 text, or holds a control character,
//!   is refused.
//! - **Untrusted:** [`project_trust`] tells the engine when the project has a layer Codex leaves
//!   out, so the chat can say so.

use std::path::Path;

use super::paths::PROJECT_DIR_NAME;
use super::settings::{CONFIG_FILE, is_trusted};
use crate::error::{BusinessError, Result};
use crate::model::ConfigFormat;
use crate::pi_family::read_document;

/// The files and folders of a project's `.codex` that Codex applies only when it trusts the
/// folder: its settings, hooks and command rules.
const PROJECT_LAYER: [&str; 3] = [CONFIG_FILE, "hooks.json", "rules"];

/// What a project's own Codex layer means for a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectTrust {
    /// The project has no layer of its own (or its `.codex` is the Codex home).
    NothingToTrust,
    /// The user's `config.toml` trusts the folder (or its git repository's root): Codex applies
    /// the layer.
    Trusted,
    /// The folder is not trusted: Codex ignores the project's layer.
    Untrusted,
}

/// Whether Codex, with its home in `codex_home`, applies the own layer of `project` (see the
/// module docs).
pub fn project_trust(codex_home: &Path, project: &Path) -> ProjectTrust {
    let layer = project.join(PROJECT_DIR_NAME);

    if layer == codex_home || !PROJECT_LAYER.iter().any(|name| layer.join(name).exists()) {
        return ProjectTrust::NothingToTrust;
    }

    let user = read_document(&codex_home.join(CONFIG_FILE), ConfigFormat::Toml);

    if is_trusted(&user, project) {
        ProjectTrust::Trusted
    } else {
        ProjectTrust::Untrusted
    }
}

/// `-c projects={"<project>"={trust_level="trusted"}}`: trusts `project` for one launch (see the
/// module docs). `invalidInput` for a path that cannot be written as a TOML string.
pub(super) fn trust_once_args(project: &Path) -> Result<[String; 2]> {
    let quoted = project
        .to_str()
        .filter(|_| project.is_absolute())
        .and_then(toml_string)
        .ok_or_else(|| {
            BusinessError::InvalidInput(
                "Codex cannot trust this folder once: its path cannot be passed to Codex".to_owned(),
            )
        })?;

    Ok([
        "-c".to_owned(),
        format!("projects={{{quoted}={{trust_level=\"trusted\"}}}}"),
    ])
}

/// `text` as a TOML basic string, `None` when it holds a control character.
pub(super) fn toml_string(text: &str) -> Option<String> {
    if text.is_empty() || text.chars().any(char::is_control) {
        return None;
    }

    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");

    Some(format!("\"{escaped}\""))
}
