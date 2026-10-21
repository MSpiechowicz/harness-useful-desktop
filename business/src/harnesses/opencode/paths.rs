//! OpenCode directory resolution, mirroring `packages/core/src/global.ts` and
//! `packages/opencode/src/config/paths.ts` (1.19.3, `xdg-basedir`), and `util/src/global.ts`
//! (2.1.2).
//!
//! - Config: `$XDG_CONFIG_HOME/opencode`, else `~/.config/opencode`, on every OS (Windows too).
//!   On OpenCode 1, `OPENCODE_CONFIG_DIR` adds one more config directory, read after the others;
//!   it never replaces this one. On OpenCode 2 it replaces it ([`global_config_dir`]).
//! - Data (sessions database, credentials, logs): `$XDG_DATA_HOME/opencode`, else
//!   `~/.local/share/opencode`.
//! - Project layer: `<project>/.opencode`.
//!
//! Relative XDG values are ignored, as the XDG specification asks; a relative
//! `OPENCODE_CONFIG_DIR` is ignored too (OpenCode would resolve it against its working directory).

use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::harness::{HarnessPaths, OpenCodeGeneration};

/// Application name used under the XDG directories.
const APP_NAME: &str = "opencode";

/// Project-local configuration directory name.
pub const PROJECT_DIR_NAME: &str = ".opencode";

/// Resolves every OpenCode directory for `env` (see the module docs). OpenCode has no profiles;
/// skills, agents, commands and themes live in the global config directory of `generation`.
pub fn paths(env: &Env, project: Option<&Path>, generation: OpenCodeGeneration) -> HarnessPaths {
    let config = global_config_dir(env, generation);

    HarnessPaths {
        config_root: config.clone(),
        agent_dir: config,
        data_root: data_dir(env),
        profile: None,
        project_dir: project.map(|project| project.join(PROJECT_DIR_NAME)),
    }
}

/// The global config directory: `$XDG_CONFIG_HOME/opencode` or `~/.config/opencode`.
pub fn config_dir(env: &Env) -> PathBuf {
    xdg_dir(env, "XDG_CONFIG_HOME", ".config").join(APP_NAME)
}

/// The global config directory `generation` reads: [`config_dir`], or on OpenCode 2 the absolute
/// directory `OPENCODE_CONFIG_DIR` names instead.
pub fn global_config_dir(env: &Env, generation: OpenCodeGeneration) -> PathBuf {
    match generation {
        OpenCodeGeneration::V1 => config_dir(env),
        OpenCodeGeneration::V2 => custom_config_dir(env).unwrap_or_else(|| config_dir(env)),
    }
}

/// The data directory: `$XDG_DATA_HOME/opencode` or `~/.local/share/opencode`.
pub fn data_dir(env: &Env) -> PathBuf {
    xdg_dir(env, "XDG_DATA_HOME", ".local/share").join(APP_NAME)
}

/// `~/.opencode`, read as a config directory after the project's (it also holds the install
/// script's `bin/`).
pub fn home_config_dir(env: &Env) -> PathBuf {
    env.home_join(PROJECT_DIR_NAME)
}

/// The directory `OPENCODE_CONFIG_DIR` names, when absolute: an extra config directory on
/// OpenCode 1, the global one on OpenCode 2.
pub fn custom_config_dir(env: &Env) -> Option<PathBuf> {
    absolute_var(env, "OPENCODE_CONFIG_DIR")
}

/// The XDG base directory `name`, else `~/<fallback>`.
fn xdg_dir(env: &Env, name: &str, fallback: &str) -> PathBuf {
    absolute_var(env, name).unwrap_or_else(|| env.home_join(fallback))
}

fn absolute_var(env: &Env, name: &str) -> Option<PathBuf> {
    env.var(name).map(PathBuf::from).filter(|dir| dir.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_and_data_default_below_home() {
        let paths = paths(
            &Env::for_tests("/home/u"),
            Some(Path::new("/work/app")),
            OpenCodeGeneration::V1,
        );

        assert_eq!(paths.config_root, PathBuf::from("/home/u/.config/opencode"));
        assert_eq!(paths.agent_dir, paths.config_root);
        assert_eq!(paths.data_root, PathBuf::from("/home/u/.local/share/opencode"));
        assert_eq!(paths.project_dir, Some(PathBuf::from("/work/app/.opencode")));
    }

    #[test]
    fn absolute_xdg_directories_move_config_and_data() {
        let config = std::env::temp_dir().join("cfg");
        let env = Env::for_tests("/home/u")
            .with_var("XDG_CONFIG_HOME", config.to_string_lossy())
            .with_var("XDG_DATA_HOME", "relative/data");

        assert_eq!(config_dir(&env), config.join("opencode"));
        assert_eq!(data_dir(&env), PathBuf::from("/home/u/.local/share/opencode"));
    }

    #[test]
    fn the_custom_config_dir_is_an_extra_layer_not_the_root() {
        let team = std::env::temp_dir().join("team");
        let env = Env::for_tests("/home/u").with_var("OPENCODE_CONFIG_DIR", team.to_string_lossy());

        let config_root = paths(&env, None, OpenCodeGeneration::V1).config_root;
        assert_eq!(config_root, PathBuf::from("/home/u/.config/opencode"));
        assert_eq!(custom_config_dir(&env), Some(team));
        assert_eq!(custom_config_dir(&env.with_var("OPENCODE_CONFIG_DIR", "team")), None);
    }

    #[test]
    fn on_opencode_2_the_custom_config_dir_replaces_the_global_one() {
        let team = std::env::temp_dir().join("team");
        let env = Env::for_tests("/home/u").with_var("OPENCODE_CONFIG_DIR", team.to_string_lossy());
        let relative = env.clone().with_var("OPENCODE_CONFIG_DIR", "team");

        assert_eq!(paths(&env, None, OpenCodeGeneration::V2).config_root, team);
        assert_eq!(
            global_config_dir(&relative, OpenCodeGeneration::V2),
            PathBuf::from("/home/u/.config/opencode")
        );
    }
}
