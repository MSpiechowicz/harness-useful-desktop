//! OpenCode's config layers, lowest precedence first (see [`super`]): which settings files and
//! config directories exist. OpenCode 2 finds them differently ([`super::discovery_v2`]).

use std::path::{Path, PathBuf};

use super::super::paths::{PROJECT_DIR_NAME, config_dir, custom_config_dir, home_config_dir};
use super::discovery_v2;
use super::read::read_config;
use crate::env::Env;
use crate::harness::OpenCodeGeneration;
use crate::model::{ConfigLayer, ExtensionScope};
use crate::pi_family::Document;

/// Settings files in every config location, lowest precedence first.
pub(super) const SETTINGS_FILES: [&str; 2] = ["opencode.json", "opencode.jsonc"];

/// The global settings file OpenCode 1 read before `opencode.json` (OpenCode 2 does not).
pub(super) const LEGACY_FILE: &str = "config.json";

/// OpenCode 1 terminal UI settings files in every config location, lowest precedence first.
const TUI_FILES: &[&str] = &["tui.json", "tui.jsonc"];

/// OpenCode 2 terminal UI settings, in the global config directory only.
const CLI_FILE: &[&str] = &["cli.json"];

/// Where a config file or directory sits; decides its layer and extension scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::harnesses::opencode) enum Place {
    /// The global config directory (`~/.config/opencode`, or `OPENCODE_CONFIG_DIR` on OpenCode 2).
    Global,
    /// The file `OPENCODE_CONFIG` names.
    EnvFile,
    /// A project directory (`opencode.json[c]` beside the code).
    Project,
    /// A `.opencode/` directory of the project.
    ProjectDir,
    /// `~/.opencode` (on OpenCode 2 only when the project walk reaches it).
    Home,
    /// The home directory itself when the project walk reaches it (`~/opencode.json`). OpenCode
    /// reads it like a project file, but it applies to every project below the home directory,
    /// so it is shown as user configuration.
    HomeRoot,
    /// The directory `OPENCODE_CONFIG_DIR` names (OpenCode 1).
    Custom,
}

impl Place {
    /// The config layer of files found here.
    pub(in crate::harnesses::opencode) fn layer(self) -> ConfigLayer {
        match self {
            Place::Project | Place::ProjectDir => ConfigLayer::Project,
            _ => ConfigLayer::User,
        }
    }

    /// The extension scope of resources found here.
    pub(in crate::harnesses::opencode) fn scope(self) -> ExtensionScope {
        match self.layer() {
            ConfigLayer::Project => ExtensionScope::Project,
            _ => ExtensionScope::User,
        }
    }
}

/// A directory OpenCode scans for agents, commands, plugins and skills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::harnesses::opencode) struct ConfigDir {
    /// Location.
    pub path: PathBuf,
    /// What kind of location it is.
    pub place: Place,
}

/// One step of OpenCode's config merge.
#[derive(Debug, Clone)]
pub(in crate::harnesses::opencode) enum Step {
    /// A settings file that exists, already parsed.
    File {
        /// Location.
        path: PathBuf,
        /// What kind of location it is.
        place: Place,
        /// The parsed content.
        document: Document,
    },
    /// A config directory, scanned for Markdown agents and commands and local plugins.
    Dir(ConfigDir),
}

/// The merge steps of one generation.
#[derive(Debug, Clone)]
pub(in crate::harnesses::opencode) struct Layers {
    /// Which OpenCode reads them: decides the keys read from each file.
    pub generation: OpenCodeGeneration,
    /// Settings files and config directories, lowest precedence first.
    pub steps: Vec<Step>,
}

/// Every settings file that exists and every config directory `generation` reads, in its merge
/// order (see [`super`]).
pub(in crate::harnesses::opencode) fn layers(
    env: &Env,
    project: Option<&Path>,
    generation: OpenCodeGeneration,
) -> Layers {
    let steps = match generation {
        OpenCodeGeneration::V1 => v1_steps(env, project),
        OpenCodeGeneration::V2 => discovery_v2::steps(env, project),
    };

    Layers { generation, steps }
}

fn v1_steps(env: &Env, project: Option<&Path>) -> Vec<Step> {
    let mut files: Vec<(PathBuf, Place)> = Vec::new();
    let global = config_dir(env);

    files.push((global.join(LEGACY_FILE), Place::Global));
    files.extend(SETTINGS_FILES.iter().map(|name| (global.join(name), Place::Global)));
    files.extend(env_config_file(env).map(|path| (path, Place::EnvFile)));

    for dir in v1_project_roots(env, project) {
        files.extend(SETTINGS_FILES.iter().map(|name| (dir.path.join(name), dir.place)));
    }

    let mut steps: Vec<Step> = files
        .into_iter()
        .filter_map(|(path, place)| file_step(path, place))
        .collect();

    for dir in v1_config_dirs(env, project) {
        if dir.place != Place::Global {
            let names = SETTINGS_FILES.iter();
            steps.extend(names.filter_map(|name| file_step(dir.path.join(name), dir.place)));
        }

        steps.push(Step::Dir(dir));
    }

    steps
}

/// The settings file at `path`, parsed, when it exists.
pub(super) fn file_step(path: PathBuf, place: Place) -> Option<Step> {
    if !path.is_file() {
        return None;
    }

    let document = read_config(&path);
    Some(Step::File { path, place, document })
}

/// The config directories `generation` scans, in its order: on OpenCode 1 the global one, every
/// project `.opencode/` from the project up to the worktree (see [`project_ancestors`]),
/// `~/.opencode` and `OPENCODE_CONFIG_DIR` (a directory listed twice keeps its first place); on
/// OpenCode 2 see [`discovery_v2::config_dirs`].
pub(in crate::harnesses::opencode) fn config_dirs(
    env: &Env,
    project: Option<&Path>,
    generation: OpenCodeGeneration,
) -> Vec<ConfigDir> {
    match generation {
        OpenCodeGeneration::V1 => v1_config_dirs(env, project),
        OpenCodeGeneration::V2 => discovery_v2::config_dirs(env, project),
    }
}

fn v1_config_dirs(env: &Env, project: Option<&Path>) -> Vec<ConfigDir> {
    let mut dirs = vec![ConfigDir {
        path: config_dir(env),
        place: Place::Global,
    }];

    if let Some(project) = project
        && project_config_enabled(env, OpenCodeGeneration::V1)
    {
        dirs.extend(project_ancestors(project).into_iter().map(|dir| {
            // `~/.opencode` keeps its user place when the walk passes the home directory.
            let place = if dir == env.home {
                Place::Home
            } else {
                Place::ProjectDir
            };

            ConfigDir {
                path: dir.join(PROJECT_DIR_NAME),
                place,
            }
        }));
    }

    dirs.push(ConfigDir {
        path: home_config_dir(env),
        place: Place::Home,
    });
    dirs.extend(custom_config_dir(env).map(|path| ConfigDir {
        path,
        place: Place::Custom,
    }));

    let mut unique: Vec<ConfigDir> = Vec::new();
    for dir in dirs {
        if !unique.iter().any(|kept| kept.path == dir.path) {
            unique.push(dir);
        }
    }

    unique
}

/// The names of the terminal UI settings files `generation` reads in a directory at `place`:
/// `tui.json[c]` everywhere on OpenCode 1, `cli.json` in the global directory on OpenCode 2.
pub(super) fn tui_names(place: Place, generation: OpenCodeGeneration) -> &'static [&'static str] {
    match (generation, place) {
        (OpenCodeGeneration::V1, _) => TUI_FILES,
        (OpenCodeGeneration::V2, Place::Global) => CLI_FILE,
        (OpenCodeGeneration::V2, _) => &[],
    }
}

/// Every terminal UI settings file that exists, lowest precedence first.
pub(in crate::harnesses::opencode) fn tui_files(
    env: &Env,
    project: Option<&Path>,
    generation: OpenCodeGeneration,
) -> Vec<PathBuf> {
    let global = config_dirs(env, project, generation).into_iter().take(1);
    let mut dirs: Vec<ConfigDir> = global.collect();
    dirs.extend(project_roots(env, project, generation));
    dirs.extend(
        config_dirs(env, project, generation)
            .into_iter()
            .filter(|dir| dir.place != Place::Global),
    );

    dirs.iter()
        .flat_map(|dir| tui_names(dir.place, generation).iter().map(|name| dir.path.join(name)))
        .filter(|path| path.is_file())
        .collect()
}

/// The project directories whose `opencode.json[c]` `generation` reads, the farthest first (see
/// [`project_ancestors`] and [`discovery_v2::project_roots`]); none when project config is off.
/// The home directory, when the walk reaches it, is [`Place::HomeRoot`].
pub(super) fn project_roots(env: &Env, project: Option<&Path>, generation: OpenCodeGeneration) -> Vec<ConfigDir> {
    match generation {
        OpenCodeGeneration::V1 => v1_project_roots(env, project),
        OpenCodeGeneration::V2 => discovery_v2::project_roots(env, project),
    }
}

fn v1_project_roots(env: &Env, project: Option<&Path>) -> Vec<ConfigDir> {
    let Some(project) = project else {
        return Vec::new();
    };

    if !project_config_enabled(env, OpenCodeGeneration::V1) {
        return Vec::new();
    }

    let mut roots = project_ancestors(project);
    roots.reverse();

    roots.into_iter().map(|path| project_root(env, path)).collect()
}

/// A walked project directory: [`Place::HomeRoot`] for the home directory.
pub(super) fn project_root(env: &Env, path: PathBuf) -> ConfigDir {
    let place = if path == env.home {
        Place::HomeRoot
    } else {
        Place::Project
    };

    ConfigDir { path, place }
}

/// `project` and its ancestors, closest first, as OpenCode 1 walks them up to the worktree
/// (`config/paths.ts`, `project/project.ts`): the repository root, the first directory containing
/// `.git` (included), else the filesystem root. The home directory is walked like any other (OMP
/// and PI stop before it).
pub(in crate::harnesses::opencode) fn project_ancestors(project: &Path) -> Vec<PathBuf> {
    ancestors_within(project, None)
}

/// [`project_ancestors`], also stopping at `boundary` (included) when no repository root comes
/// first. Tests pass a temporary directory so the walk never leaves it.
fn ancestors_within(project: &Path, boundary: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    for dir in project.ancestors() {
        dirs.push(dir.to_path_buf());

        let repository_root = dir.join(".git").exists();
        if repository_root || boundary == Some(dir) {
            break;
        }
    }

    dirs
}

/// False when `OPENCODE_DISABLE_PROJECT_CONFIG` is `true` or `1`, as OpenCode's flag reads it.
/// OpenCode 2 reads `OPENCODE_CONFIG_PROJECT_DISABLE` first, when set.
pub(super) fn project_config_enabled(env: &Env, generation: OpenCodeGeneration) -> bool {
    let preferred = match generation {
        OpenCodeGeneration::V1 => None,
        OpenCodeGeneration::V2 => env.var("OPENCODE_CONFIG_PROJECT_DISABLE"),
    };

    let value = preferred
        .or_else(|| env.var("OPENCODE_DISABLE_PROJECT_CONFIG"))
        .unwrap_or_default();
    !(value.eq_ignore_ascii_case("true") || value == "1")
}

/// The absolute file `OPENCODE_CONFIG` names (a relative one depends on OpenCode's working
/// directory, so it is skipped).
pub(super) fn env_config_file(env: &Env) -> Option<PathBuf> {
    env.var("OPENCODE_CONFIG")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::OpenCodeGeneration::{V1, V2};

    #[test]
    fn the_project_switch_reads_true_or_one() {
        let enabled_with = |value: &str| {
            let env = Env::for_tests("/home/u").with_var("OPENCODE_DISABLE_PROJECT_CONFIG", value);
            project_config_enabled(&env, V1)
        };

        assert!(project_config_enabled(&Env::for_tests("/home/u"), V1));
        assert!(!enabled_with("TRUE"));
        assert!(!enabled_with("1"));
        assert!(enabled_with("yes"));
    }

    #[test]
    fn opencode_2_prefers_its_own_project_switch() {
        let env = Env::for_tests("/home/u")
            .with_var("OPENCODE_DISABLE_PROJECT_CONFIG", "1")
            .with_var("OPENCODE_CONFIG_PROJECT_DISABLE", "false");

        assert!(project_config_enabled(&env, V2));
        assert!(!project_config_enabled(&env, V1));
    }

    #[test]
    fn the_walk_stops_at_the_repository_root_including_it() {
        let temp = tempfile::TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        let project = repo.join("app").join("web");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir(repo.join(".git")).unwrap();

        assert_eq!(
            ancestors_within(&project, Some(temp.path())),
            [project.clone(), repo.join("app"), repo]
        );
    }

    #[test]
    fn without_a_repository_the_walk_goes_up_to_the_boundary() {
        let temp = tempfile::TempDir::new().unwrap();
        let home = temp.path().join("home");
        let project = home.join("notes");
        std::fs::create_dir_all(&project).unwrap();

        assert_eq!(
            ancestors_within(&project, Some(temp.path())),
            [project.clone(), home, temp.path().to_path_buf()]
        );
    }

    #[test]
    fn without_a_boundary_the_walk_ends_at_the_filesystem_root() {
        let temp = tempfile::TempDir::new().unwrap();
        let walked = project_ancestors(temp.path());

        assert_eq!(walked[0], temp.path());
        let last = walked.last().unwrap();
        assert!(last.parent().is_none() || last.join(".git").exists(), "{last:?}");
    }
}
