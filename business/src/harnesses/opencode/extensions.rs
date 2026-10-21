//! The OpenCode extensions view: skills, commands, agents, plugins, MCP servers and themes, read
//! from the layers in [`super::config`] the way OpenCode 1.19.3 or 2.1.2 merges them (later layers
//! win). OpenCode 2 also reads its native keys (`plugins`, `mcp.servers`, `agents`, `commands`)
//! beside the OpenCode 1 ones.
//!
//! Nothing is installed or run: npm plugins are listed by their spec, never resolved, and MCP
//! servers are never started.

mod agents;
mod mcp;
mod plugins;

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

pub(super) use self::plugins::{plugin_entry_id, plugin_sources, plugins};
use super::config::{
    Layers, Place, Step, command_entries, config_dirs, layers, project_ancestors, read_config, tui_files, v2_walk,
};
use super::paths::{PROJECT_DIR_NAME, global_config_dir, home_config_dir};
use crate::env::Env;
use crate::harness::OpenCodeGeneration;
use crate::model::{ExtensionScope, ExtensionsView, HarnessId, PromptTemplateInfo, SkillInfo, ThemeInfo};
use crate::pi_family::frontmatter::MAX_MARKDOWN_BYTES;
use crate::pi_family::resources::walk_tree;
use crate::pi_family::{
    SkillRoot, builtin_themes, dedupe_skills, parse_frontmatter, scan_skills, text_field, themes_in_dir,
};
use crate::security::{SafeReader, redact_text};

/// OpenCode finds `**/SKILL.md` at any depth; this caps the walk.
const SKILL_DEPTH: usize = 4;

/// Caps the `**/*.md` walk for agents and commands.
const MARKDOWN_DEPTH: usize = 4;

/// Themes shipped with OpenCode (`DEFAULT_THEMES` of its terminal UI).
const BUILTIN_THEMES: &[&str] = &[
    "aura",
    "ayu",
    "catppuccin",
    "catppuccin-frappe",
    "catppuccin-macchiato",
    "cobalt2",
    "cursor",
    "dracula",
    "everforest",
    "flexoki",
    "github",
    "gruvbox",
    "kanagawa",
    "material",
    "matrix",
    "mercury",
    "monokai",
    "nightowl",
    "nord",
    "one-dark",
    "osaka-jade",
    "opencode",
    "orng",
    "lucent-orng",
    "palenight",
    "rosepine",
    "solarized",
    "synthwave84",
    "tokyonight",
    "vesper",
    "vercel",
    "zenburn",
    "carbonfox",
];

const DEFAULT_THEME: &str = "opencode";

/// See the module docs.
pub fn extensions_view(env: &Env, project: Option<&Path>, generation: OpenCodeGeneration) -> ExtensionsView {
    let layers = layers(env, project, generation);

    ExtensionsView {
        harness: HarnessId::OpenCode,
        plugins: plugins(&layers),
        marketplaces: Vec::new(),
        skills: skills(env, project, generation),
        prompt_templates: prompt_templates(&layers),
        agents: agents::agents(&layers),
        mcp_servers: mcp::mcp_servers(&layers),
        themes: themes(env, project, generation),
        packages: Vec::new(),
        extension_modules: Vec::new(),
        notices: Vec::new(),
    }
}

/// OpenCode reads `~/.claude/skills`, `~/.agents/skills`, then `.claude/skills` and
/// `.agents/skills` from the project up (to the worktree on OpenCode 1, see [`project_ancestors`];
/// along the whole config walk on OpenCode 2, see [`v2_walk`]), then `skill/` and `skills/` of every
/// config directory; a later skill of the same name wins.
fn skills(env: &Env, project: Option<&Path>, generation: OpenCodeGeneration) -> Vec<SkillInfo> {
    let mut roots = Vec::new();
    let mut add = |dir: PathBuf, scope| roots.push(SkillRoot::labeled(env, project, dir, scope));

    for external in [".claude", ".agents"] {
        add(env.home_join(external).join("skills"), ExtensionScope::User);
    }

    let walked = match generation {
        OpenCodeGeneration::V1 => project.map(project_ancestors).unwrap_or_default(),
        OpenCodeGeneration::V2 => v2_walk(env, project),
    };
    for dir in walked {
        for external in [".claude", ".agents"] {
            add(dir.join(external).join("skills"), walked_scope(env, &dir));
        }
    }

    for dir in config_dirs(env, project, generation) {
        for name in ["skill", "skills"] {
            add(dir.path.join(name), dir.place.scope());
        }
    }

    // `dedupe_skills` keeps the first skill of each name, so the winners go first.
    roots.reverse();

    let found = roots.iter().flat_map(|root| scan_skills(root, SKILL_DEPTH)).collect();
    dedupe_skills(found)
}

/// The scope of what the project walk finds in `dir`: the home directory holds user resources.
fn walked_scope(env: &Env, dir: &Path) -> ExtensionScope {
    if dir == env.home {
        ExtensionScope::User
    } else {
        ExtensionScope::Project
    }
}

/// One command definition: name, description, defining file and its place.
type CommandDefinition = (String, Option<String>, PathBuf, Place);

/// The `command` key of each settings file (and `commands` on OpenCode 2), and the Markdown files
/// in `command/` and `commands/` of each config directory (named by their path below that
/// directory). A later definition takes over the file and scope and keeps an earlier description
/// it does not set.
fn prompt_templates(layers: &Layers) -> Vec<PromptTemplateInfo> {
    let mut templates: Vec<PromptTemplateInfo> = Vec::new();

    for step in &layers.steps {
        let found: Vec<CommandDefinition> = match step {
            Step::File { path, place, document } => document
                .value()
                .map(|value| command_entries(value, layers.generation))
                .unwrap_or_default()
                .into_iter()
                .map(|(name, command)| (name, description(&command), path.clone(), *place))
                .collect(),

            Step::Dir(dir) => markdown_files(&dir.path, &["command", "commands"], true)
                .into_iter()
                .filter_map(|(name, file)| {
                    let fields = Value::Object(markdown_fields(&file)?);
                    Some((name, description(&fields), file, dir.place))
                })
                .collect(),
        };

        for (name, description, path, place) in found {
            let template = PromptTemplateInfo {
                name,
                description,
                argument_hint: None,
                scope: place.scope(),
                path: path.to_string_lossy().into_owned(),
            };

            match templates.iter_mut().find(|existing| existing.name == template.name) {
                Some(existing) => {
                    let description = template.description.clone().or(existing.description.take());
                    *existing = PromptTemplateInfo {
                        description,
                        ..template
                    };
                }
                None => templates.push(template),
            }
        }
    }

    templates.sort_by(|left, right| left.name.cmp(&right.name));
    templates
}

/// Built-in themes plus `themes/*.json` in the global config directory and every project
/// `.opencode/` (a later theme of the same name wins). OpenCode 1 walks up to the worktree, then
/// reads `~/.opencode`; OpenCode 2 walks up to the filesystem root, the farthest directory last.
/// The active theme is the last `theme` (or `theme.name`) set in a terminal UI settings file, else
/// `opencode`.
fn themes(env: &Env, project: Option<&Path>, generation: OpenCodeGeneration) -> Vec<ThemeInfo> {
    let active = active_theme(env, project, generation);
    let is_active = |name: &str| name == active;

    let mut dirs = vec![(global_config_dir(env, generation), ExtensionScope::User)];
    let walked: Vec<PathBuf> = match (generation, project) {
        (_, None) => Vec::new(),
        (OpenCodeGeneration::V1, Some(project)) => project_ancestors(project),
        (OpenCodeGeneration::V2, Some(project)) => project.ancestors().map(Path::to_path_buf).collect(),
    };
    dirs.extend(
        walked
            .into_iter()
            .map(|dir| (dir.join(PROJECT_DIR_NAME), walked_scope(env, &dir))),
    );
    if generation == OpenCodeGeneration::V1 {
        dirs.push((home_config_dir(env), ExtensionScope::User));
    }

    let mut themes = builtin_themes(BUILTIN_THEMES, &is_active);

    for (dir, scope) in dirs {
        for theme in themes_in_dir(&dir.join("themes"), scope, &is_active) {
            match themes.iter_mut().find(|existing| existing.name == theme.name) {
                Some(existing) => *existing = theme,
                None => themes.push(theme),
            }
        }
    }

    themes
}

/// `theme` (or `tui.theme`, or `theme.name` as OpenCode 2 writes it) of the last terminal UI
/// settings file that sets one.
fn active_theme(env: &Env, project: Option<&Path>, generation: OpenCodeGeneration) -> String {
    tui_files(env, project, generation)
        .iter()
        .filter_map(|path| {
            let document = read_config(path);
            let nested = document.get("tui").and_then(|tui| tui.get("theme"));
            let theme = document.get("theme").or(nested)?;

            theme.get("name").unwrap_or(theme).as_str().map(str::to_owned)
        })
        .next_back()
        .unwrap_or_else(|| DEFAULT_THEME.to_owned())
}

/// The `description` of a definition, with secrets in the text masked.
fn description(definition: &Value) -> Option<String> {
    text_field(definition, "description").map(|text| redact_text(&text))
}

/// `(name, path)` of the `*.md` files in `<dir>/<sub>` for each of `subdirs`, named by their path
/// below `<sub>` without the extension (`agents/review/deep.md` is `review/deep`). Subdirectories
/// are walked only when `nested`, and symlink loops are not followed (see [`walk_tree`]).
fn markdown_files(dir: &Path, subdirs: &[&str], nested: bool) -> Vec<(String, PathBuf)> {
    let depth = if nested { MARKDOWN_DEPTH } else { 1 };
    let mut found = Vec::new();

    for sub in subdirs {
        let root = dir.join(sub);

        walk_tree(&root, depth, &mut |entry| {
            if entry.is_dir {
                return true;
            }

            if entry.is_file
                && entry.name.ends_with(".md")
                && let Some(name) = markdown_name(&root, &entry.path)
            {
                found.push((name, entry.path.clone()));
            }

            false
        });
    }

    found
}

/// The path of `file` below `root` without `.md`, `/`-separated on every platform.
fn markdown_name(root: &Path, file: &Path) -> Option<String> {
    let parts: Option<Vec<&str>> = file
        .strip_prefix(root)
        .ok()?
        .components()
        .map(|part| part.as_os_str().to_str())
        .collect();

    parts?.join("/").strip_suffix(".md").map(str::to_owned)
}

/// The frontmatter fields of a Markdown definition: empty without a frontmatter block, `None` when
/// the file is unreadable or the block is not a YAML mapping (OpenCode skips those files).
fn markdown_fields(path: &Path) -> Option<Map<String, Value>> {
    let text = SafeReader::read_to_string(path, MAX_MARKDOWN_BYTES).ok()?;

    if !text.trim_start_matches('\u{feff}').starts_with("---") {
        return Some(Map::new());
    }

    match parse_frontmatter(&text)? {
        Value::Object(fields) => Some(fields),
        _ => None,
    }
}
