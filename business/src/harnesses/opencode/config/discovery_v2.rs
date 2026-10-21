//! How OpenCode 2 finds its config (`core/src/config/discovery.ts` and `core/src/config.ts`, 2.1.2).
//!
//! The global directory is `OPENCODE_CONFIG_DIR` when set, else `~/.config/opencode`
//! ([`global_config_dir`]); it holds `opencode.json` and `opencode.jsonc`, never `config.json`.
//! The project walk goes from the project up to the filesystem root (no repository stop) and
//! finds `opencode.json[c]` files and `.opencode/` directories. Merge order, lowest first: the
//! global files and directory, the file `OPENCODE_CONFIG` names, the project files (farthest
//! first), then each project `.opencode/` (farthest first) with its files. `~/.opencode` is not a
//! layer of its own: it is read only when the walk reaches the home directory. The walk is
//! skipped when project config is off or the project is itself a global root (the global
//! directory, `~/.claude` or `~/.agents`). Inline `OPENCODE_CONFIG_CONTENT` and the managed
//! directory are not read.

use std::path::{Path, PathBuf};

use super::super::paths::{PROJECT_DIR_NAME, global_config_dir};
use super::layers::{ConfigDir, Place, SETTINGS_FILES, Step, env_config_file, file_step, project_config_enabled};
use crate::env::Env;
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
/// global directory is never one of them.
pub(super) fn project_roots(env: &Env, project: Option<&Path>) -> Vec<ConfigDir> {
    let global = global_config_dir(env, GENERATION);
    let mut walked = walk(env, project);
    walked.reverse();

    walked
        .into_iter()
        .filter(|dir| *dir != global)
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
/// project, when project config is off, or when the project is a global root.
pub(in crate::harnesses::opencode) fn walk(env: &Env, project: Option<&Path>) -> Vec<PathBuf> {
    let Some(project) = project else {
        return Vec::new();
    };

    if !project_config_enabled(env, GENERATION) {
        return Vec::new();
    }

    let global_roots = [
        global_config_dir(env, GENERATION),
        env.home_join(".claude"),
        env.home_join(".agents"),
    ];
    if global_roots.iter().any(|root| root == project) {
        return Vec::new();
    }

    project.ancestors().map(Path::to_path_buf).collect()
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
    fn a_global_root_or_switched_off_project_config_is_not_walked() {
        let home = std::env::temp_dir().join("home");
        let env = Env::for_tests(&home);

        assert!(walk(&env, Some(&home.join(".claude"))).is_empty());
        assert!(walk(&env, Some(&home.join(".config/opencode"))).is_empty());

        let off = env.with_var("OPENCODE_DISABLE_PROJECT_CONFIG", "true");
        assert!(walk(&off, Some(&home.join("work"))).is_empty());
    }
}
