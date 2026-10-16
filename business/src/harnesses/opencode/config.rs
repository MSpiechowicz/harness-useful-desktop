//! The OpenCode configuration view and the file layers the extensions view merges, following
//! `config/config.ts`, `config/paths.ts` and `config/tui.ts` (1.19.3).
//!
//! OpenCode merges, lowest precedence first: the global `config.json` (legacy), `opencode.json`
//! and `opencode.jsonc`; the file `OPENCODE_CONFIG` names; the project's `opencode.json[c]` from
//! the worktree down to the project; then each config directory after the global one: every
//! `.opencode/` from the project up to the worktree, `~/.opencode/` and
//! `OPENCODE_CONFIG_DIR`. `OPENCODE_DISABLE_PROJECT_CONFIG` skips the project files and
//! `.opencode/` directories. Terminal UI settings (`tui.json[c]`) sit beside each of them. Every
//! file is JSON with comments.
//!
//! Not shown: inline `OPENCODE_CONFIG_CONTENT` and `OPENCODE_PERMISSION` (a notice says when they
//! are set, never what they hold), remote and account config, and the managed directory
//! (`/etc/opencode` and its macOS and Windows counterparts). `{env:NAME}` references show the
//! variable name and `{file:path}` references show as written; neither is resolved, so the
//! referenced file is never read.
//!
//! [`layers`] finds the files and directories; this module renders them as redacted sections.

mod layers;

use std::path::Path;

use serde_json::Value;

use self::layers::{LEGACY_FILE, SETTINGS_FILES, TUI_FILES, env_config_file, project_config_enabled, project_roots};
pub(super) use self::layers::{
    Place, Step, config_dirs, format_of, merge_steps, project_ancestors, read_config, tui_files,
};
use super::paths::config_dir;
use crate::env::Env;
use crate::model::{ConfigField, ConfigSection, ConfigValue, ConfigView, HarnessId, Notice, NoticeLevel};
use crate::pi_family::{SectionText, origin_label, section_from, source};
use crate::security::{RedactionContext, Redactor, redact_text};

const CREDENTIALS_NOTICE: &str = "Credentials (auth.json, mcp-auth.json) and the session database (opencode.db) are \
                                  never read. {env:…} and {file:…} references are shown, never resolved.";

const PROJECT_CONFIG_OFF_NOTICE: &str =
    "OPENCODE_DISABLE_PROJECT_CONFIG is on, so OpenCode ignores this project's settings and .opencode directories.";

/// See the module docs.
pub fn config_view(env: &Env, project: Option<&Path>) -> ConfigView {
    let mut sections = global_sections(env, project);

    if let Some(path) = env_config_file(env) {
        sections.push(file_section(env, project, &path, Place::EnvFile, FileKind::Settings));
    }

    for dir in project_roots(env, project) {
        sections.extend(dir_sections(env, project, &dir.path, dir.place));
    }

    for dir in config_dirs(env, project) {
        if dir.place != Place::Global {
            sections.extend(dir_sections(env, project, &dir.path, dir.place));
        }
    }

    ConfigView {
        harness: HarnessId::OpenCode,
        sections,
        notices: notices(env, project),
    }
}

/// What a config file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileKind {
    Settings,
    Legacy,
    Tui,
}

/// The global files that exist. `opencode.json` is listed even when missing, unless another
/// settings file exists, so the view shows where settings would go.
fn global_sections(env: &Env, project: Option<&Path>) -> Vec<ConfigSection> {
    let dir = config_dir(env);
    let mut files = vec![(dir.join(LEGACY_FILE), FileKind::Legacy)];
    files.extend(SETTINGS_FILES.iter().map(|name| (dir.join(name), FileKind::Settings)));
    files.retain(|(path, _)| path.is_file());

    if files.is_empty() {
        files.push((dir.join(SETTINGS_FILES[0]), FileKind::Settings));
    }

    let mut sections: Vec<ConfigSection> = files
        .iter()
        .map(|(path, kind)| file_section(env, project, path, Place::Global, *kind))
        .collect();

    sections.extend(tui_sections(env, project, &dir, Place::Global));
    sections
}

/// The settings and terminal UI files of a project or config directory that exist.
fn dir_sections(env: &Env, project: Option<&Path>, dir: &Path, place: Place) -> Vec<ConfigSection> {
    let mut sections: Vec<ConfigSection> = SETTINGS_FILES
        .iter()
        .map(|name| dir.join(name))
        .filter(|path| path.is_file())
        .map(|path| file_section(env, project, &path, place, FileKind::Settings))
        .collect();

    sections.extend(tui_sections(env, project, dir, place));
    sections
}

fn tui_sections(env: &Env, project: Option<&Path>, dir: &Path, place: Place) -> Vec<ConfigSection> {
    TUI_FILES
        .iter()
        .map(|name| dir.join(name))
        .filter(|path| path.is_file())
        .map(|path| file_section(env, project, &path, place, FileKind::Tui))
        .collect()
}

fn file_section(env: &Env, project: Option<&Path>, path: &Path, place: Place, kind: FileKind) -> ConfigSection {
    let id = format!("{}:{}", place_id(place), origin_label(env, project, path));
    let (title, description) = section_text(place, kind);
    let text = SectionText {
        id: &id,
        title,
        description,
    };

    let file = source(path, place.layer(), None, format_of(path));
    let mut section = section_from(text, file, &read_config(path));

    mask_mcp_commands(&mut section.entries);
    section
}

/// Applies [`command_words`] to the `mcp.<name>.command` lists of a redacted file.
fn mask_mcp_commands(entries: &mut [ConfigField]) {
    let servers = entries
        .iter_mut()
        .filter(|field| field.key == "mcp")
        .filter_map(|field| match &mut field.value {
            ConfigValue::Map(servers) => Some(servers),
            _ => None,
        })
        .flatten();

    for server in servers {
        let ConfigValue::Map(fields) = &mut server.value else {
            continue;
        };

        for field in fields.iter_mut().filter(|field| field.key == "command") {
            if let ConfigValue::List(words) = &mut field.value {
                *words = command_words(std::mem::take(words));
            }
        }
    }
}

/// Masks secrets in the already redacted words of an MCP command line: credentials and
/// `--token=...` assignments inside a word, and the whole word after a flag whose name looks
/// secret (`--api-key <value>`).
pub(super) fn command_words(words: Vec<ConfigValue>) -> Vec<ConfigValue> {
    let mut masked: Vec<ConfigValue> = Vec::with_capacity(words.len());

    for word in words {
        let flag = match masked.last() {
            Some(ConfigValue::Text(previous)) => flag_name(previous),
            _ => None,
        };

        let word = match (word, flag) {
            (ConfigValue::Text(text), Some(flag)) => {
                Redactor.redact_at(&Value::String(text), &mut RedactionContext::at([flag]))
            }
            (ConfigValue::Text(text), None) => ConfigValue::Text(redact_text(&text)),
            (other, _) => other,
        };

        masked.push(word);
    }

    masked
}

/// The name of a `--flag` / `-f` word that carries no value of its own.
fn flag_name(word: &str) -> Option<String> {
    let name = word.trim_start_matches('-');

    if name.len() == word.len() || name.is_empty() || name.contains('=') {
        return None;
    }

    Some(name.to_owned())
}

fn place_id(place: Place) -> &'static str {
    match place {
        Place::Global => "user",
        Place::EnvFile => "env",
        Place::Project | Place::ProjectDir => "project",
        Place::Home | Place::HomeRoot => "home",
        Place::Custom => "custom",
    }
}

/// Title and description of a file.
fn section_text(place: Place, kind: FileKind) -> (&'static str, &'static str) {
    match (place, kind) {
        (Place::Global, FileKind::Legacy) => (
            "Legacy settings",
            "The older global settings file. OpenCode reads it before opencode.json.",
        ),
        (Place::Global, FileKind::Tui) => ("Terminal UI", "Theme, keybindings and other terminal UI settings."),
        (Place::Global, _) => (
            "Settings",
            "Global OpenCode settings: models, providers, agents, permissions and MCP servers.",
        ),

        (Place::EnvFile, _) => (
            "Custom settings",
            "The file OPENCODE_CONFIG names. It overrides the global settings.",
        ),

        (Place::Project, FileKind::Tui) => ("Project terminal UI", "Terminal UI settings for this project."),
        (Place::Project, _) => ("Project settings", "Overrides OpenCode applies in this project."),

        (Place::ProjectDir, FileKind::Tui) => (
            "Project terminal UI (.opencode)",
            "Terminal UI settings from a .opencode directory of this project.",
        ),
        (Place::ProjectDir, _) => (
            "Project settings (.opencode)",
            "Overrides from a .opencode directory of this project, applied after opencode.json.",
        ),

        (Place::Home, FileKind::Tui) => ("Terminal UI (~/.opencode)", "Terminal UI settings in ~/.opencode."),
        (Place::Home, _) => (
            "Settings (~/.opencode)",
            "Settings in ~/.opencode. They override the global and project settings.",
        ),

        (Place::HomeRoot, FileKind::Tui) => (
            "Home terminal UI",
            "Terminal UI settings in your home directory. OpenCode reads them for projects below it.",
        ),
        (Place::HomeRoot, _) => (
            "Home settings",
            "Settings in your home directory. OpenCode reads them for projects below it.",
        ),

        (Place::Custom, FileKind::Tui) => (
            "Terminal UI (OPENCODE_CONFIG_DIR)",
            "Terminal UI settings in the directory OPENCODE_CONFIG_DIR names.",
        ),
        (Place::Custom, _) => (
            "Settings (OPENCODE_CONFIG_DIR)",
            "Settings in the directory OPENCODE_CONFIG_DIR names. OpenCode applies them last.",
        ),
    }
}

/// Environment overrides (by name only), the project switch and the credentials notice.
fn notices(env: &Env, project: Option<&Path>) -> Vec<Notice> {
    let mut notices = Vec::new();

    let overrides = [
        ("OPENCODE_CONFIG_CONTENT", "these files"),
        ("OPENCODE_PERMISSION", "the permissions in these files"),
    ];
    for (name, what) in overrides {
        if env.is_set(name) {
            notices.push(Notice {
                level: NoticeLevel::Warning,
                message: format!("{name} is set in the environment and overrides {what}. Its value is not shown."),
            });
        }
    }

    if project.is_some() && !project_config_enabled(env) {
        notices.push(info(PROJECT_CONFIG_OFF_NOTICE));
    }

    notices.push(info(CREDENTIALS_NOTICE));
    notices
}

fn info(message: &str) -> Notice {
    Notice {
        level: NoticeLevel::Info,
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SecretKind;

    fn texts(words: &[&str]) -> Vec<ConfigValue> {
        words.iter().map(|word| ConfigValue::Text((*word).to_owned())).collect()
    }

    #[test]
    fn words_after_secret_flags_and_secret_assignments_are_hidden() {
        let masked = command_words(texts(&[
            "npx",
            "server",
            "--api-key",
            "abc",
            "--port",
            "8080",
            "--token=abc",
        ]));

        assert_eq!(masked[3], ConfigValue::Secret(SecretKind::Value));
        assert_eq!(masked[5], ConfigValue::Text("8080".to_owned()));
        assert_eq!(masked[6], ConfigValue::Text("--token=••••".to_owned()));
    }
}
