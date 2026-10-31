//! The model a new Claude Code chat starts on from Harness Skills' model roles:
//! `<config dir>/harness-useful-skills/models.yml` (`useful-skills/models.yml` before Harness Skills
//! 0.4.0, read when only it exists), `modelRoles: {default: "<model>[:<effort>]", …}`
//! (Harness Skills' `claude/models.js`).
//!
//! - **File:** read through [`SafeReader`], at most [`MAX_BYTES`], as YAML. It must hold
//!   `modelRoles` and nothing else, every role a known one with a valid selector, as Harness Skills
//!   requires; otherwise the whole file is ignored.
//! - **Selector:** an optional `anthropic/` prefix, then the model (`inherit`, an alias of
//!   [`ALIASES`] or `claude-` with lowercase letters, digits, `.` and `-`), then optionally `:` and
//!   an effort of [`EFFORTS`]. These single tokens are what reaches the command line.
//! - **Default:** the `default` role, unless it is `inherit` (the chat starts on the CLI's own
//!   default then). A missing, unreadable or invalid file starts the chat on the CLI default too.

use std::path::Path;

use serde_json::Value;

use crate::ecosystem::skills_paths::skills_data_path;
use crate::error::BusinessError;
use crate::model::ConfigFormat;
use crate::pi_family::config_files::parse_text;
use crate::security::SafeReader;

/// Largest models file read (Harness Skills' own cap).
const MAX_BYTES: u64 = 16 * 1024;

/// The roles Harness Skills knows.
const ROLES: [&str; 9] = [
    "research",
    "frontend",
    "backend",
    "implementation",
    "review",
    "security",
    "plan",
    "assess",
    "default",
];

/// The role a chat starts on.
const DEFAULT_ROLE: &str = "default";

/// The model names that are not a `claude-` id.
const ALIASES: [&str; 4] = ["opus", "sonnet", "haiku", "fable"];

/// The model that leaves the choice to the CLI.
const INHERIT: &str = "inherit";

/// The efforts a selector may name.
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// The prefix an OMP-style selector may carry.
const PROVIDER_PREFIX: &str = "anthropic/";

/// The prefix of a full model id.
const MODEL_ID_PREFIX: &str = "claude-";

/// Longest model id after [`MODEL_ID_PREFIX`].
const MAX_MODEL_ID_LEN: usize = 81;

/// A model and effort to start on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Selection {
    /// A Claude Code model id or alias, never `inherit`.
    pub(super) model: String,
    pub(super) effort: Option<String>,
}

/// The default role of the models file in `config_dir`, when it names a model (see the module docs).
pub(super) fn default_selection(config_dir: &Path) -> Option<Selection> {
    let path = skills_data_path(config_dir, &["models.yml"]);

    let text = match SafeReader::read_to_string(&path, MAX_BYTES) {
        Ok(text) => text,
        Err(BusinessError::NotFound(_)) => return None,
        Err(error) => {
            tracing::debug!(%error, "Harness Skills' model roles are unreadable");
            return None;
        }
    };

    let selection = parse_text(&text, ConfigFormat::Yaml)
        .ok()
        .and_then(|document| default_role(&document));

    if selection.is_none() {
        tracing::debug!("Harness Skills' model roles name no valid default model");
    }

    selection.filter(|selection| selection.model != INHERIT)
}

/// The `default` role of a valid `document`, `inherit` included.
fn default_role(document: &Value) -> Option<Selection> {
    let top = document.as_object()?;
    let roles = top.get("modelRoles")?.as_object()?;

    if top.len() != 1 {
        return None;
    }

    let mut default = None;

    for (role, raw) in roles {
        if !ROLES.contains(&role.as_str()) {
            return None;
        }

        let selection = selector(raw.as_str()?)?;

        if role == DEFAULT_ROLE {
            default = Some(selection);
        }
    }

    default
}

/// `<model>[:<effort>]` with an optional `anthropic/` prefix.
fn selector(raw: &str) -> Option<Selection> {
    let raw = raw.strip_prefix(PROVIDER_PREFIX).unwrap_or(raw);
    let mut parts = raw.split(':');

    let model = parts.next()?;
    let effort = parts.next();

    if parts.next().is_some() || !is_model(model) {
        return None;
    }

    if effort.is_some_and(|effort| !EFFORTS.contains(&effort)) {
        return None;
    }

    Some(Selection {
        model: model.to_owned(),
        effort: effort.map(str::to_owned),
    })
}

/// `inherit`, an alias or `claude-` then a lowercase letter or digit and up to 80 more of those,
/// `.` or `-`.
fn is_model(model: &str) -> bool {
    if model == INHERIT || ALIASES.contains(&model) {
        return true;
    }

    let Some(id) = model.strip_prefix(MODEL_ID_PREFIX) else {
        return false;
    };

    let lower_alphanumeric = |character: char| character.is_ascii_lowercase() || character.is_ascii_digit();

    id.len() <= MAX_MODEL_ID_LEN
        && id.chars().next().is_some_and(lower_alphanumeric)
        && id
            .chars()
            .all(|character| lower_alphanumeric(character) || character == '.' || character == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(model: &str, effort: Option<&str>) -> Option<Selection> {
        Some(Selection {
            model: model.to_owned(),
            effort: effort.map(str::to_owned),
        })
    }

    fn parsed(text: &str) -> Option<Selection> {
        default_role(&parse_text(text, ConfigFormat::Yaml).ok()?)
    }

    #[test]
    fn reads_the_default_role_as_harness_skills_writes_it() {
        let text = "# comment\nmodelRoles:\n  research: claude-haiku-5-5:high\n  default: claude-opus-5-5:medium\n";
        assert_eq!(parsed(text), selection("claude-opus-5-5", Some("medium")));

        assert_eq!(
            parsed("modelRoles:\n  default: \"anthropic/sonnet\"\n"),
            selection("sonnet", None)
        );
        assert_eq!(parsed("modelRoles:\n  default: inherit\n"), selection("inherit", None));
        assert_eq!(parsed("modelRoles:\n  plan: opus\n"), None, "no default role");

        assert_eq!(
            parsed("modelRoles:\n  default: sonnet:low\n  assess: inherit\n"),
            selection("sonnet", Some("low")),
            "the assess role is a known one"
        );
    }

    #[test]
    fn refuses_a_file_harness_skills_refuses() {
        for text in [
            "modelRoles:\n  default: opus\nother: 1\n",
            "modelRoles:\n  default: opus\n  writer: opus\n",
            "modelRoles:\n  default: opus\n  plan: gpt-5\n",
            "modelRoles: opus\n",
            "default: opus\n",
            "modelRoles:\n  default: 5\n",
        ] {
            assert_eq!(parsed(text), None, "{text}");
        }
    }

    #[test]
    fn a_selector_is_single_tokens_of_a_strict_set() {
        assert_eq!(
            selector("claude-sonnet-5-5:high"),
            selection("claude-sonnet-5-5", Some("high"))
        );
        assert_eq!(selector("claude-opus-4.8"), selection("claude-opus-4.8", None));

        for raw in [
            "--x",
            "claude-opus-5-5 --x",
            "opus:high:max",
            "opus:extreme",
            "Claude-opus",
            "claude-",
            "claude--x",
            "claude-opus_5",
            "gpt-5",
            "opus:",
            "",
        ] {
            assert_eq!(selector(raw), None, "{raw}");
        }

        let long = format!("claude-{}", "a".repeat(MAX_MODEL_ID_LEN + 1));
        assert_eq!(selector(&long), None);
    }

    fn write(config: &std::path::Path, folder: &str, text: &str) {
        let path = config.join(folder).join("models.yml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn the_models_file_is_read_from_the_new_folder_else_the_old_one() {
        let config = tempfile::tempdir().unwrap();
        assert_eq!(default_selection(config.path()), None, "no file");

        write(config.path(), "useful-skills", "modelRoles:\n  default: sonnet:low\n");
        assert_eq!(
            default_selection(config.path()),
            selection("sonnet", Some("low")),
            "only the folder before Harness Skills 0.4.0"
        );

        write(
            config.path(),
            "harness-useful-skills",
            "modelRoles:\n  default: opus:high\n",
        );
        assert_eq!(
            default_selection(config.path()),
            selection("opus", Some("high")),
            "the new folder wins"
        );

        std::fs::remove_dir_all(config.path().join("useful-skills")).unwrap();
        assert_eq!(default_selection(config.path()), selection("opus", Some("high")));
    }

    #[test]
    fn an_invalid_new_file_is_not_replaced_by_the_old_one() {
        let config = tempfile::tempdir().unwrap();
        write(config.path(), "useful-skills", "modelRoles:\n  default: sonnet\n");
        write(config.path(), "harness-useful-skills", "modelRoles: nope\n");

        assert_eq!(default_selection(config.path()), None);
    }
}
