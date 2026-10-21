//! How OpenCode 2 finds its config (`core/src/config/discovery.ts` and `core/src/config.ts`, 2.1.2).
//!
//! The global directory is `OPENCODE_CONFIG_DIR` when set, else `~/.config/opencode`
//! ([`global_config_dir`]); it holds `opencode.json` and `opencode.jsonc`, never `config.json`.
//! The project walk goes from the project up to the filesystem root (no repository stop) and
//! finds `opencode.json[c]` files and `.opencode/` directories. Merge order, lowest first: the
//! global files and directory, the file `OPENCODE_CONFIG` names, the project files (farthest
//! first), then each project `.opencode/` (farthest first) with its files. `~/.opencode` is not a
//! layer of its own: it is read only when the walk reaches the home directory. The walk is
//! skipped when project config is off or the project is itself a config directory (the global
//! one, or the system one, [`system_config_dir`]). Inline `OPENCODE_CONFIG_CONTENT` and the
//! system (managed) directory are not read.

use std::path::{Path, PathBuf};

use super::super::paths::{PROJECT_DIR_NAME, global_config_dir};
use super::layers::{ConfigDir, Place, SETTINGS_FILES, Step, env_config_file, file_step, project_config_enabled};
use crate::env::{Env, OsKind};
use crate::harness::OpenCodeGeneration;

const GENERATION: OpenCodeGeneration = OpenCodeGeneration::V2;

/// Every settings file that exists and every config directory, in merge order (see the module
/// docs).
pub(super) fn steps(env: &Env, project: Option<&Path>) -> Vec<Step> {
    let mut steps = Vec::new();
    let global = global_config_dir(env, GENERATION);

    steps.extend(settings_steps(&global, Place::Global));
    steps.push(Step::Dir(ConfigDir {
        path: global,
        place: Place::Global,
    }));

    steps.extend(env_config_file(env).and_then(|path| file_step(path, Place::EnvFile)));

    for dir in project_roots(env, project) {
        steps.extend(settings_steps(&dir.path, dir.place));
    }

    for dir in project_dirs(env, project) {
        steps.extend(settings_steps(&dir.path, dir.place));
        steps.push(Step::Dir(dir));
    }

    steps
}

fn settings_steps(dir: &Path, place: Place) -> impl Iterator<Item = Step> {
    SETTINGS_FILES
        .iter()
        .filter_map(move |name| file_step(dir.join(name), place))
}

/// The global directory, then every project `.opencode/` the walk finds, the farthest first.
pub(super) fn config_dirs(env: &Env, project: Option<&Path>) -> Vec<ConfigDir> {
    let global = ConfigDir {
        path: global_config_dir(env, GENERATION),
        place: Place::Global,
    };

    std::iter::once(global).chain(project_dirs(env, project)).collect()
}

/// The walked directories whose `opencode.json[c]` OpenCode 2 reads, the farthest first; the
/// global and the system config directories are never among them.
pub(super) fn project_roots(env: &Env, project: Option<&Path>) -> Vec<ConfigDir> {
    let config_dirs = [system_config_dir(env), global_config_dir(env, GENERATION)];
    let mut walked = walk(env, project);
    walked.reverse();

    walked
        .into_iter()
        .filter(|dir| !config_dirs.contains(dir))
        .map(|dir| super::layers::project_root(env, dir))
        .collect()
}

/// The `.opencode/` directories of the walk, the farthest first; `~/.opencode` keeps its user
/// place, and the global directory is never one of them.
fn project_dirs(env: &Env, project: Option<&Path>) -> Vec<ConfigDir> {
    let global = global_config_dir(env, GENERATION);
    let mut walked = walk(env, project);
    walked.reverse();

    walked
        .into_iter()
        .map(|dir| {
            let place = if dir == env.home {
                Place::Home
            } else {
                Place::ProjectDir
            };

            ConfigDir {
                path: dir.join(PROJECT_DIR_NAME),
                place,
            }
        })
        .filter(|dir| dir.path != global)
        .collect()
}

/// `project` and every ancestor up to the filesystem root, closest first; none without a
/// project, when project config is off, or when the project is the global or the system config
/// directory (as `discovery.ts` skips it; `~/.claude` and `~/.agents` are walked).
pub(in crate::harnesses::opencode) fn walk(env: &Env, project: Option<&Path>) -> Vec<PathBuf> {
    let Some(project) = project else {
        return Vec::new();
    };

    if !project_config_enabled(env, GENERATION) {
        return Vec::new();
    }

    let config_dirs = [system_config_dir(env), global_config_dir(env, GENERATION)];
    if config_dirs.iter().any(|dir| dir == project) {
        return Vec::new();
    }

    project.ancestors().map(Path::to_path_buf).collect()
}

/// OpenCode 2's system config directory: `/etc/opencode`, `/Library/Application Support/opencode`
/// on macOS, `%ProgramData%\opencode` on Windows (`systemDirectory` in `discovery.ts`).
fn system_config_dir(env: &Env) -> PathBuf {
    match env.os {
        OsKind::Linux => PathBuf::from("/etc/opencode"),
        OsKind::MacOs => PathBuf::from("/Library/Application Support/opencode"),
        OsKind::Windows => PathBuf::from(env.var("ProgramData").unwrap_or("C:\\ProgramData")).join("opencode"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_walk_goes_past_the_repository_root() {
        let temp = tempfile::TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        let project = repo.join("app");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(&project).unwrap();

        let walked = walk(&Env::for_tests(temp.path().join("home")), Some(&project));

        assert_eq!(walked[..3], [project.clone(), repo, temp.path().to_path_buf()]);
        assert!(walked.last().unwrap().parent().is_none());
    }

    #[test]
    fn a_config_directory_or_switched_off_project_config_is_not_walked() {
        let home = std::env::temp_dir().join("home");
        let env = Env::for_tests(&home).with_os(OsKind::Linux);

        assert!(walk(&env, Some(&home.join(".config/opencode"))).is_empty());
        assert!(walk(&env, Some(Path::new("/etc/opencode"))).is_empty());

        let off = env.with_var("OPENCODE_DISABLE_PROJECT_CONFIG", "true");
        assert!(walk(&off, Some(&home.join("work"))).is_empty());
    }

    #[test]
    fn claude_and_agents_folders_are_walked_like_any_project() {
        let home = std::env::temp_dir().join("home");
        let env = Env::for_tests(&home);

        for folder in [".claude", ".agents"] {
            let project = home.join(folder);

            assert_eq!(walk(&env, Some(&project))[..2], [project.clone(), home.clone()]);
        }
    }

    #[test]
    fn the_system_config_directory_follows_the_os() {
        let env = Env::for_tests("/home/u");

        assert_eq!(
            system_config_dir(&env.clone().with_os(OsKind::MacOs)),
            PathBuf::from("/Library/Application Support/opencode")
        );
        assert_eq!(
            system_config_dir(&env.clone().with_os(OsKind::Windows).with_var("ProgramData", "D:\\Data")),
            PathBuf::from("D:\\Data").join("opencode")
        );
    }
}
