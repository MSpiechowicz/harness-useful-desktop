//! OpenCode's config layers, lowest precedence first (see [`super`]): which settings files and
//! config directories exist, and how a settings file is read.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::super::paths::{PROJECT_DIR_NAME, config_dir, custom_config_dir, home_config_dir};
use crate::env::Env;
use crate::model::{ConfigFormat, ConfigLayer, ExtensionScope};
use crate::pi_family::{Document, read_document};

/// Settings files in every config location, lowest precedence first.
pub(super) const SETTINGS_FILES: [&str; 2] = ["opencode.json", "opencode.jsonc"];

/// The global settings file OpenCode read before `opencode.json`.
pub(super) const LEGACY_FILE: &str = "config.json";

/// Terminal UI settings files in every config location, lowest precedence first.
pub(super) const TUI_FILES: [&str; 2] = ["tui.json", "tui.jsonc"];

/// Where a config file or directory sits; decides its layer and extension scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::harnesses::opencode) enum Place {
    /// The global config directory (`~/.config/opencode`).
    Global,
    /// The file `OPENCODE_CONFIG` names.
    EnvFile,
    /// A project directory (`opencode.json[c]` beside the code).
    Project,
    /// A `.opencode/` directory of the project.
    ProjectDir,
    /// `~/.opencode`.
    Home,
    /// The home directory itself when the project walk reaches it (`~/opencode.json`). OpenCode
    /// reads it like a project file, but it applies to every project below the home directory,
    /// so it is shown as user configuration.
    HomeRoot,
    /// The directory `OPENCODE_CONFIG_DIR` names.
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

/// Every settings file that exists and every config directory, in OpenCode's merge order (see
/// [`super`]).
pub(in crate::harnesses::opencode) fn merge_steps(env: &Env, project: Option<&Path>) -> Vec<Step> {
    let mut files: Vec<(PathBuf, Place)> = Vec::new();
    let global = config_dir(env);

    files.push((global.join(LEGACY_FILE), Place::Global));
    files.extend(SETTINGS_FILES.iter().map(|name| (global.join(name), Place::Global)));
    files.extend(env_config_file(env).map(|path| (path, Place::EnvFile)));

    for dir in project_roots(env, project) {
        files.extend(SETTINGS_FILES.iter().map(|name| (dir.path.join(name), dir.place)));
    }

    let mut steps: Vec<Step> = files
        .into_iter()
        .filter_map(|(path, place)| file_step(path, place))
        .collect();

    for dir in config_dirs(env, project) {
        if dir.place != Place::Global {
            let names = SETTINGS_FILES.iter();
            steps.extend(names.filter_map(|name| file_step(dir.path.join(name), dir.place)));
        }

        steps.push(Step::Dir(dir));
    }

    steps
}

fn file_step(path: PathBuf, place: Place) -> Option<Step> {
    if !path.is_file() {
        return None;
    }

    let document = read_config(&path);
    Some(Step::File { path, place, document })
}

/// The config directories in OpenCode order: the global one, every project `.opencode/` from the
/// project up to the worktree (see [`project_ancestors`]), `~/.opencode` and `OPENCODE_CONFIG_DIR`.
/// A directory listed twice keeps its first place.
pub(in crate::harnesses::opencode) fn config_dirs(env: &Env, project: Option<&Path>) -> Vec<ConfigDir> {
    let mut dirs = vec![ConfigDir {
        path: config_dir(env),
        place: Place::Global,
    }];

    if let Some(project) = project
        && project_config_enabled(env)
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

/// Every terminal UI settings file that exists, lowest precedence first.
pub(in crate::harnesses::opencode) fn tui_files(env: &Env, project: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = vec![config_dir(env)];
    dirs.extend(project_roots(env, project).into_iter().map(|dir| dir.path));
    dirs.extend(
        config_dirs(env, project)
            .into_iter()
            .filter(|dir| dir.place != Place::Global)
            .map(|dir| dir.path),
    );

    dirs.iter()
        .flat_map(|dir| TUI_FILES.iter().map(move |name| dir.join(name)))
        .filter(|path| path.is_file())
        .collect()
}

/// Reads an OpenCode settings file: JSON with comments, `{env:NAME}` values shown by name.
pub(in crate::harnesses::opencode) fn read_config(path: &Path) -> Document {
    match read_document(path, ConfigFormat::Jsonc) {
        Document::Parsed(value) => Document::Parsed(env_names(value)),
        other => other,
    }
}

/// `jsonc` for `*.jsonc` files, else `json` (OpenCode accepts comments in both).
pub(in crate::harnesses::opencode) fn format_of(path: &Path) -> ConfigFormat {
    match path.extension() {
        Some(extension) if extension == "jsonc" => ConfigFormat::Jsonc,
        _ => ConfigFormat::Json,
    }
}

/// Rewrites every string that is exactly `{env:NAME}` to `${NAME}`, which the redactor shows as
/// a reference to `NAME` without reading it. Other text, `{file:...}` included, is left as written.
fn env_names(value: Value) -> Value {
    match value {
        Value::String(text) => match env_placeholder(&text) {
            Some(name) => Value::String(format!("${{{name}}}")),
            None => Value::String(text),
        },
        Value::Array(items) => Value::Array(items.into_iter().map(env_names).collect()),
        Value::Object(entries) => {
            Value::Object(entries.into_iter().map(|(key, item)| (key, env_names(item))).collect())
        }
        other => other,
    }
}

/// `NAME` when `text` is exactly `{env:NAME}` and `NAME` is a plain variable name.
fn env_placeholder(text: &str) -> Option<&str> {
    let name = text.strip_prefix("{env:")?.strip_suffix('}')?;
    let mut chars = name.chars();

    let starts_well = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
    let continues_well = chars.all(|next| next.is_ascii_alphanumeric() || next == '_');

    (starts_well && continues_well).then_some(name)
}

/// The project directories whose `opencode.json[c]` OpenCode reads, the farthest first (see
/// [`project_ancestors`]); none when project config is off. The home directory, when the walk
/// reaches it, is [`Place::HomeRoot`].
pub(super) fn project_roots(env: &Env, project: Option<&Path>) -> Vec<ConfigDir> {
    let Some(project) = project else {
        return Vec::new();
    };

    if !project_config_enabled(env) {
        return Vec::new();
    }

    let mut roots = project_ancestors(project);
    roots.reverse();

    roots
        .into_iter()
        .map(|path| {
            let place = if path == env.home {
                Place::HomeRoot
            } else {
                Place::Project
            };

            ConfigDir { path, place }
        })
        .collect()
}

/// `project` and its ancestors, closest first, as OpenCode walks them up to the worktree
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
pub(super) fn project_config_enabled(env: &Env) -> bool {
    let value = env.var("OPENCODE_DISABLE_PROJECT_CONFIG").unwrap_or_default();
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
    use serde_json::json;

    use super::*;

    #[test]
    fn only_whole_env_placeholders_become_references() {
        let value = json!({
            "apiKey": "{env:ANTHROPIC_API_KEY}",
            "list": ["{env:_X1}", "{env:1BAD}", "pre {env:Y}", "{file:~/.secrets/key}"],
        });

        assert_eq!(
            env_names(value),
            json!({
                "apiKey": "${ANTHROPIC_API_KEY}",
                "list": ["${_X1}", "{env:1BAD}", "pre {env:Y}", "{file:~/.secrets/key}"],
            })
        );
    }

    #[test]
    fn the_project_switch_reads_true_or_one() {
        let enabled_with = |value: &str| {
            let env = Env::for_tests("/home/u").with_var("OPENCODE_DISABLE_PROJECT_CONFIG", value);
            project_config_enabled(&env)
        };

        assert!(project_config_enabled(&Env::for_tests("/home/u")));
        assert!(!enabled_with("TRUE"));
        assert!(!enabled_with("1"));
        assert!(enabled_with("yes"));
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
