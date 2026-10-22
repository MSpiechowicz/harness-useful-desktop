//! Claude Code directory resolution (Claude Code 2.1.304).
//!
//! - Config, data and sessions: `$CLAUDE_CONFIG_DIR`, else `~/.claude`, on every OS. A relative
//!   `CLAUDE_CONFIG_DIR` is ignored (Claude would resolve it against its working directory).
//! - Global file (user MCP servers, per-project state): `$CLAUDE_CONFIG_DIR/.claude.json` when the
//!   variable is set, else `~/.claude.json`, next to the config directory and not inside it.
//! - Sessions of a project: `<config>/projects/<slug>`, where the slug is the project's absolute
//!   path with every character that is not an ASCII letter or digit replaced by `-`.
//! - Project layer: `<project>/.claude`.

use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::harness::HarnessPaths;

/// The default config directory name below the home directory, and the project layer's name.
pub const PROJECT_DIR_NAME: &str = ".claude";

/// The global file's name.
const GLOBAL_FILE_NAME: &str = ".claude.json";

/// Resolves every Claude Code directory for `env` (see the module docs). Claude has no profiles;
/// settings, plugins, skills, agents and sessions all live in the config directory.
pub fn paths(env: &Env, project: Option<&Path>) -> HarnessPaths {
    let config = config_dir(env);

    HarnessPaths {
        config_root: config.clone(),
        agent_dir: config.clone(),
        data_root: config,
        profile: None,
        project_dir: project.map(|project| project.join(PROJECT_DIR_NAME)),
    }
}

/// The config directory: the absolute `CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub fn config_dir(env: &Env) -> PathBuf {
    custom_config_dir(env).unwrap_or_else(|| env.home_join(PROJECT_DIR_NAME))
}

/// The global file: `.claude.json` inside the absolute `CLAUDE_CONFIG_DIR`, else `~/.claude.json`.
pub fn global_config_file(env: &Env) -> PathBuf {
    match custom_config_dir(env) {
        Some(dir) => dir.join(GLOBAL_FILE_NAME),
        None => env.home_join(GLOBAL_FILE_NAME),
    }
}

/// The folder holding the session files of `project`: `<config>/projects/<slug>`.
pub fn sessions_dir(env: &Env, project: &Path) -> PathBuf {
    config_dir(env).join("projects").join(project_slug(project))
}

/// `project` as Claude names its session folder: every UTF-16 code unit that is not an ASCII
/// letter or digit becomes `-`, as Claude's `replace(/[^a-zA-Z0-9]/g, "-")` does.
pub fn project_slug(project: &Path) -> String {
    project
        .to_string_lossy()
        .encode_utf16()
        .map(|unit| match char::from_u32(u32::from(unit)) {
            Some(character) if character.is_ascii_alphanumeric() => character,
            _ => '-',
        })
        .collect()
}

/// The directory `CLAUDE_CONFIG_DIR` names, when absolute.
fn custom_config_dir(env: &Env) -> Option<PathBuf> {
    env.var("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slug_replaces_every_character_but_ascii_letters_and_digits() {
        assert_eq!(project_slug(Path::new("/home/u/.config/x")), "-home-u--config-x");
        assert_eq!(project_slug(Path::new("/w/Zażółć_1")), "-w-Za-----1");
        assert_eq!(project_slug(Path::new("/w/a😀b")), "-w-a--b");
    }
}
