//! The OpenCode configuration view and the file layers the extensions view merges, following
//! `config/config.ts`, `config/paths.ts` and `config/tui.ts` (1.19.3). OpenCode 2 finds its files
//! differently (see [`discovery_v2`]): no `config.json`, `OPENCODE_CONFIG_DIR` replaces the global
//! directory, the project walk goes up to the filesystem root, and its terminal UI settings are
//! the global `cli.json`. Each file is shown as written; [`keys`] reads the keys of each generation.
//!
//! OpenCode 1 merges, lowest precedence first: the global `config.json` (legacy), `opencode.json`
//! and `opencode.jsonc`; the file `OPENCODE_CONFIG` names; the project's `opencode.json[c]` from
//! the worktree down to the project; then each config directory after the global one: every
//! `.opencode/` from the project up to the worktree, `~/.opencode/` and
//! `OPENCODE_CONFIG_DIR`. `OPENCODE_DISABLE_PROJECT_CONFIG` skips the project files and
//! `.opencode/` directories. Terminal UI settings (`tui.json[c]`) sit beside each of them. Every
//! file is JSON with comments.
//!
//! Not shown: inline `OPENCODE_CONFIG_CONTENT` and, on OpenCode 1, `OPENCODE_PERMISSION` (a
//! notice says when they are set, never what they hold), remote and account config, and the managed directory
//! (`/etc/opencode` and its macOS and Windows counterparts). `{env:NAME}` references show the
//! variable name and `{file:path}` references show as written; neither is resolved, so the
//! referenced file is never read.
//!
//! [`layers`] finds the files and directories; this module renders them as redacted sections.

mod discovery_v2;
mod keys;
mod layers;
mod read;

use std::path::Path;

pub(crate) use self::keys::{PluginEntry, mcp_servers};
pub(super) use self::keys::{command_entries, object_entries, plugin_entries, replace};
use self::layers::{LEGACY_FILE, SETTINGS_FILES, env_config_file, project_config_enabled, project_roots, tui_names};
pub(super) use self::layers::{Layers, Place, Step, config_dirs, layers, project_ancestors, tui_files};
pub(super) use self::read::{format_of, read_config};
use super::paths::global_config_dir;
use crate::env::Env;
use crate::harness::OpenCodeGeneration;
use crate::model::{ConfigField, ConfigSection, ConfigValue, ConfigView, HarnessId, Notice, NoticeLevel};
pub(super) use crate::pi_family::mcp_config::command_words;
use crate::pi_family::{SectionText, origin_label, section_from, source};
pub(super) use discovery_v2::walk as v2_walk;

const CREDENTIALS_NOTICE: &str = "Credentials (auth.json, mcp-auth.json) and the session database (opencode.db) are \
                                  never read. {env:…} and {file:…} references are shown, never resolved.";

const PROJECT_CONFIG_OFF_NOTICE: &str =
    "OPENCODE_DISABLE_PROJECT_CONFIG is on, so OpenCode ignores this project's settings and .opencode directories.";

/// See the module docs.
pub fn config_view(env: &Env, project: Option<&Path>, generation: OpenCodeGeneration) -> ConfigView {
    let view = View {
        env,
        project,
        generation,
    };
    let mut sections = view.global_sections();

    if let Some(path) = env_config_file(env) {
        sections.push(view.file_section(&path, Place::EnvFile, FileKind::Settings));
    }

    for dir in project_roots(env, project, generation) {
        sections.extend(view.dir_sections(&dir.path, dir.place));
    }

    for dir in config_dirs(env, project, generation) {
        if dir.place != Place::Global {
            sections.extend(view.dir_sections(&dir.path, dir.place));
        }
    }

    ConfigView {
        harness: HarnessId::OpenCode,
        sections,
        notices: notices(env, project, generation),
    }
}

/// What the view is built for.
#[derive(Clone, Copy)]
struct View<'a> {
    env: &'a Env,
    project: Option<&'a Path>,
    generation: OpenCodeGeneration,
}

/// What a config file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileKind {
    Settings,
    Legacy,
    Tui,
}

impl View<'_> {
    /// The global files that exist. `opencode.json` is listed even when missing, unless another
    /// settings file exists, so the view shows where settings would go. Only OpenCode 1 reads the
    /// legacy `config.json`.
    fn global_sections(self) -> Vec<ConfigSection> {
        let dir = global_config_dir(self.env, self.generation);
        let mut files = Vec::new();

        if self.generation == OpenCodeGeneration::V1 {
            files.push((dir.join(LEGACY_FILE), FileKind::Legacy));
        }
        files.extend(SETTINGS_FILES.iter().map(|name| (dir.join(name), FileKind::Settings)));
        files.retain(|(path, _)| path.is_file());

        if files.is_empty() {
            files.push((dir.join(SETTINGS_FILES[0]), FileKind::Settings));
        }

        let mut sections: Vec<ConfigSection> = files
            .iter()
            .map(|(path, kind)| self.file_section(path, Place::Global, *kind))
            .collect();

        sections.extend(self.tui_sections(&dir, Place::Global));
        sections
    }

    /// The settings and terminal UI files of a project or config directory that exist.
    fn dir_sections(self, dir: &Path, place: Place) -> Vec<ConfigSection> {
        let mut sections: Vec<ConfigSection> = SETTINGS_FILES
            .iter()
            .map(|name| dir.join(name))
            .filter(|path| path.is_file())
            .map(|path| self.file_section(&path, place, FileKind::Settings))
            .collect();

        sections.extend(self.tui_sections(dir, place));
        sections
    }

    fn tui_sections(self, dir: &Path, place: Place) -> Vec<ConfigSection> {
        tui_names(place, self.generation)
            .iter()
            .map(|name| dir.join(name))
            .filter(|path| path.is_file())
            .map(|path| self.file_section(&path, place, FileKind::Tui))
            .collect()
    }

    fn file_section(self, path: &Path, place: Place, kind: FileKind) -> ConfigSection {
        let id = format!("{}:{}", place_id(place), origin_label(self.env, self.project, path));
        let (title, description) = section_text(place, kind, self.generation);
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
}

/// Applies [`command_words`] to the `mcp.<name>.command` and `mcp.servers.<name>.command` lists of
/// a redacted file.
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
        if server.key == "servers"
            && let ConfigValue::Map(nested) = &mut server.value
        {
            mask_server_commands(nested.iter_mut());
        }

        mask_server_commands(std::iter::once(server));
    }
}

/// Masks the `command` list of each server field.
fn mask_server_commands<'a>(servers: impl Iterator<Item = &'a mut ConfigField>) {
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
fn section_text(place: Place, kind: FileKind, generation: OpenCodeGeneration) -> (&'static str, &'static str) {
    if generation == OpenCodeGeneration::V2 {
        match (place, kind) {
            (Place::Global, FileKind::Tui) => {
                return (
                    "Terminal UI",
                    "Theme, keybindings and other terminal UI settings (cli.json).",
                );
            }
            (Place::Home, FileKind::Settings) => {
                return (
                    "Settings (~/.opencode)",
                    "Settings in ~/.opencode. OpenCode reads them for projects below your home directory.",
                );
            }
            _ => {}
        }
    }

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

/// Environment overrides (by name only), the project switch and the credentials notice. OpenCode 2
/// has no `OPENCODE_PERMISSION`.
fn notices(env: &Env, project: Option<&Path>, generation: OpenCodeGeneration) -> Vec<Notice> {
    let mut notices = Vec::new();

    let mut overrides = vec![("OPENCODE_CONFIG_CONTENT", "these files")];
    if generation == OpenCodeGeneration::V1 {
        overrides.push(("OPENCODE_PERMISSION", "the permissions in these files"));
    }
    for (name, what) in overrides {
        if env.is_set(name) {
            notices.push(Notice {
                level: NoticeLevel::Warning,
                message: format!("{name} is set in the environment and overrides {what}. Its value is not shown."),
            });
        }
    }

    if project.is_some() && !project_config_enabled(env, generation) {
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
