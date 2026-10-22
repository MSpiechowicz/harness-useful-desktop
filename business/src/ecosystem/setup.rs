//! What the "Recommended setup" step offers (`setup_status`): the dashboard and memory binaries,
//! and per harness the Harness Skills and the dashboard's, the memory server's and the app's own
//! (`harness-desktop mcp`) MCP servers.
//!
//! Everything here only reads: the dashboard's and the memory server's version probes, the OMP and
//! Claude Code plugin and marketplace registries, PI's user settings, the Harness Skills' OpenCode install record (see
//! [`super::skills_record`]), where `node` is (their installer needs it) and the user MCP files. The MCP files of a harness are those of
//! its user agent directory ([`Harness::paths`] without a project), so with an OMP profile active
//! only that profile's files count. OpenCode's is its global config file, Claude Code's the
//! `mcpServers` of its global file ([`mcp_target`]).

mod github;

use std::path::PathBuf;

use self::github::same_repository;
use super::catalog::{SKILLS_PLUGIN, supported_on};
use super::desktop_register::{current_app_command, desktop_mcp_state};
use super::installer::{Companion, Installed, installer};
use super::mcp_merge::Shape;
use super::mcp_register::{mcp_state, memory_mcp_state};
use super::skills_record::opencode_skills_version;
use crate::env::{Env, OsKind};
use crate::harness::{Harness, HarnessRegistry, McpTarget, OpenCodeGeneration, SkillsStrategy};
use crate::harnesses::omp::{installed_plugins, marketplaces};
use crate::harnesses::pi::user_has_package;
use crate::harnesses::{claude, opencode};
use crate::model::{
    DashboardSetup, EcosystemId, ExtensionAction, HarnessId, HarnessSetup, HarnessSummary, InstallScope, McpRefusal,
    McpState, RecommendedSetup, SkillsSetup, SkillsUnavailableCode,
};
use crate::process::{ProcessRunner, ResolvedBinary, resolve_binary};

/// The setup step's view of this machine. `summaries` are the latest harness detections (a
/// harness without one counts as not installed). Reads files; call it from a blocking-capable
/// context.
pub async fn setup_status(
    env: &Env,
    runner: &dyn ProcessRunner,
    registry: &HarnessRegistry,
    summaries: &[HarnessSummary],
) -> RecommendedSetup {
    let dashboard = binary_setup(env, runner, Companion::Dashboard).await;
    let memory = memory_server_setup(env, runner).await;

    let harnesses = registry
        .all()
        .map(|harness| {
            let installed = summaries
                .iter()
                .find(|summary| summary.info.id == harness.id())
                .is_some_and(|summary| summary.detection.installed);

            HarnessSetup {
                harness: harness.id(),
                installed,
                skills: skills_setup(env, harness),
                mcp: harness_mcp_state(env, harness, mcp_state),
                memory_mcp: harness_mcp_state(env, harness, memory_mcp_state),
                desktop_mcp: harness_mcp_state(env, harness, app_mcp_state),
            }
        })
        .collect();

    RecommendedSetup {
        dashboard,
        memory,
        harnesses,
    }
}

/// The file a harness's companion MCP servers are registered in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpFile {
    /// The file written (only read for a [`Shape::ClaudeCli`] file, which the harness's CLI
    /// changes, see [`McpFile::via_cli`]).
    pub path: PathBuf,
    /// Files that can define servers too; only read, to find an existing entry.
    pub also_read: Vec<PathBuf>,
    /// How the files hold their servers.
    pub shape: Shape,
}

impl McpFile {
    /// Whether servers are added to and removed from this file through the harness's CLI
    /// ([`super::mcp_cli`]) rather than by the app.
    pub fn via_cli(&self) -> bool {
        !self.shape.app_writes()
    }
}

/// The file the setup step and project memory register servers in for `harness`; `None` when the
/// harness has none.
///
/// OpenCode merges every global config file it finds. The app writes the one OpenCode itself
/// would edit (`global_settings_file` of the OpenCode adapter), creating `opencode.json` when
/// there is none, and only reads the others, to find an existing entry.
///
/// - OpenCode 1: `config.json` and the other settings file of the config directory,
///   `opencode.json[c]` in `~/.opencode` and in `OPENCODE_CONFIG_DIR`, and the file
///   `OPENCODE_CONFIG` names.
/// - OpenCode 2 (whose global directory is `OPENCODE_CONFIG_DIR` when set, and which reads no
///   `config.json`): the other settings file of its global directory and the file
///   `OPENCODE_CONFIG` names. Not `opencode.json[c]` in the home directory or in `~/.opencode`:
///   its project walk reads those only for projects below the home directory, so a server found
///   only there would not load elsewhere (Share setup still checks them, see
///   [`share_mcp_target`]).
pub fn mcp_target(env: &Env, harness: &dyn Harness) -> Option<McpFile> {
    match harness.companion_setup().mcp {
        McpTarget::UserMcpJson { .. } => {
            let (path, also_read) = mcp_target_paths(env, harness)?;

            Some(McpFile {
                path,
                also_read,
                shape: Shape::McpServers,
            })
        }
        McpTarget::OpenCodeConfig(generation) => Some(opencode_file(env, generation)),
        McpTarget::ClaudeCli => Some(McpFile {
            path: claude::global_config_file(env),
            also_read: Vec::new(),
            shape: Shape::ClaudeCli,
        }),
        McpTarget::Unsupported => None,
    }
}

/// The `mcp.json` of a [`McpTarget::UserMcpJson`] harness and the files it also reads; `None` for
/// any other harness. Share setup reads and writes only these (`mcpServers`) files.
pub fn mcp_target_paths(env: &Env, harness: &dyn Harness) -> Option<(PathBuf, Vec<PathBuf>)> {
    let McpTarget::UserMcpJson { also_read } = harness.companion_setup().mcp else {
        return None;
    };

    let agent_dir = harness.paths(env, None).agent_dir;
    let others = also_read.iter().map(|name| agent_dir.join(name)).collect();

    Some((agent_dir.join("mcp.json"), others))
}

/// OpenCode's global config file and the other user config files it reads (see [`mcp_target`]).
fn opencode_file(env: &Env, generation: OpenCodeGeneration) -> McpFile {
    let config = opencode::global_config_dir(env, generation);
    let path = opencode::global_settings_file(env, generation);

    let (global_names, shape, extra_dirs): (&[&str], Shape, Vec<PathBuf>) = match generation {
        OpenCodeGeneration::V1 => (
            &["config.json", "opencode.json", "opencode.jsonc"],
            Shape::OpenCode,
            std::iter::once(opencode::home_config_dir(env))
                .chain(opencode::custom_config_dir(env))
                .collect(),
        ),
        OpenCodeGeneration::V2 => (&["opencode.json", "opencode.jsonc"], Shape::OpenCodeV2, Vec::new()),
    };

    let candidates = global_names
        .iter()
        .map(|name| config.join(name))
        .chain(
            extra_dirs
                .iter()
                .flat_map(|dir| [dir.join("opencode.json"), dir.join("opencode.jsonc")]),
        )
        .chain(env_config_file(env));

    let mut also_read: Vec<PathBuf> = Vec::new();
    for candidate in candidates {
        if candidate != path && !also_read.contains(&candidate) {
            also_read.push(candidate);
        }
    }

    McpFile { path, also_read, shape }
}

/// The file Share setup merges `harness`'s servers into, and every file it checks for a server of
/// the same name: [`mcp_target`], and on OpenCode 2 also `opencode.json[c]` in the home directory
/// and in `~/.opencode`, which its project walk reads for every project below the home directory
/// (a new entry would merge with a server there).
pub fn share_mcp_target(env: &Env, harness: &dyn Harness) -> Option<McpFile> {
    let mut file = mcp_target(env, harness)?;

    if harness.companion_setup().mcp != McpTarget::OpenCodeConfig(OpenCodeGeneration::V2) {
        return Some(file);
    }

    let home_walk = [env.home.clone(), opencode::home_config_dir(env)]
        .into_iter()
        .flat_map(|dir| [dir.join("opencode.json"), dir.join("opencode.jsonc")]);

    for candidate in home_walk {
        if candidate != file.path && !file.also_read.contains(&candidate) {
            file.also_read.push(candidate);
        }
    }

    Some(file)
}

/// The file `OPENCODE_CONFIG` names, when absolute (a relative one depends on OpenCode's working
/// directory). It is only ever read: the user points OpenCode at it, so the app does not write it.
fn env_config_file(env: &Env) -> Option<PathBuf> {
    env.var("OPENCODE_CONFIG")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// The `harness-memory` binary: whether and which version is installed, and its installer.
pub async fn memory_server_setup(env: &Env, runner: &dyn ProcessRunner) -> DashboardSetup {
    binary_setup(env, runner, Companion::Memory).await
}

async fn binary_setup(env: &Env, runner: &dyn ProcessRunner, companion: Companion) -> DashboardSetup {
    let found = companion.installed(env, runner).await;

    let (id, side_effects) = match companion {
        Companion::Dashboard => (EcosystemId::Dashboard, dashboard_side_effects(env.os)),
        Companion::Memory => (EcosystemId::Memory, memory_side_effects(env.os)),
        Companion::OpenCodeSkills => (EcosystemId::Skills, skills_side_effects(env.os)),
    };

    // A memory server without a version is never registered or run, so the installer is offered.
    let installed = match companion {
        Companion::Dashboard => found != Installed::Missing,
        Companion::Memory | Companion::OpenCodeSkills => found.version().is_some(),
    };

    DashboardSetup {
        installed,
        version: found.version().map(str::to_owned),
        supported_on_this_os: supported_on(id, env.os),
        command: installer(env, companion).command.to_owned(),
        side_effects: side_effects.iter().map(|line| (*line).to_owned()).collect(),
    }
}

/// What the memory server's installer of `os` changes, per its script (`install.sh`,
/// `install.ps1`).
fn memory_side_effects(os: OsKind) -> &'static [&'static str] {
    match os {
        OsKind::Linux | OsKind::MacOs => &[
            "Installs the harness-memory command to ~/.local/bin, replacing an older one there.",
            "Appends a line that puts ~/.local/bin on your PATH to your shell profile, if it is not on PATH yet.",
            "Running memory servers keep working; each harness starts the new version next time.",
        ],
        OsKind::Windows => &[
            r"Puts the harness-memory command in %LOCALAPPDATA%\Programs\harness-memory and adds that folder to your user PATH.",
            "Running memory servers keep working; each harness starts the new version next time.",
        ],
    }
}

/// What the Harness Skills' OpenCode installer of `os` changes, per its script (`install.sh`,
/// `install.ps1`). OpenCode keeps its config folder below the home folder on Windows too.
fn skills_side_effects(os: OsKind) -> &'static [&'static str] {
    match os {
        OsKind::Linux | OsKind::MacOs => &[
            "Installs the Harness Skills plugin, skills, agents and commands into the OpenCode config folder (~/.config/opencode).",
            "Adds the useful_skills MCP server to opencode.json in that folder.",
            "Needs Node.js 22 or newer.",
            "Restart OpenCode and running OpenCode chats to load the skills.",
            "Updates come from /useful-skills update in OpenCode.",
        ],
        OsKind::Windows => &[
            r"Installs the Harness Skills plugin, skills, agents and commands into the OpenCode config folder (%USERPROFILE%\.config\opencode).",
            "Adds the useful_skills MCP server to opencode.json in that folder.",
            "Needs Node.js 22 or newer.",
            "Restart OpenCode and running OpenCode chats to load the skills.",
            "Updates come from /useful-skills update in OpenCode.",
        ],
    }
}

/// What the dashboard's installer of `os` changes, per its script (`install.sh`, `install.ps1`).
fn dashboard_side_effects(os: OsKind) -> &'static [&'static str] {
    match os {
        OsKind::Linux => &[
            "Installs the Harness Dashboard app to ~/.local/lib/harness-dashboard and the harness-dashboard command to ~/.local/bin.",
            "Adds Harness Dashboard to your applications menu.",
            "Appends a line that puts ~/.local/bin on your PATH to your shell profile, if it is not on PATH yet.",
            "Stops a running dashboard server or app so the new version can start; `harness-dashboard mcp` keeps running.",
        ],
        OsKind::MacOs => &[
            "Installs Harness Dashboard.app to ~/Applications and the harness-dashboard command to ~/.local/bin.",
            "Appends a line that puts ~/.local/bin on your PATH to your shell profile, if it is not on PATH yet.",
            "Stops a running dashboard server or app so the new version can start; `harness-dashboard mcp` keeps running.",
        ],
        OsKind::Windows => &[
            "Installs the Harness Dashboard app for your user, with a Start menu shortcut and an entry under Settings > Apps.",
            r"Puts the harness-dashboard command in %LOCALAPPDATA%\Programs\harness-dashboard and adds that folder to your user PATH.",
            "Stops a running dashboard server or app in this session; `harness-dashboard mcp` keeps running.",
        ],
    }
}

fn skills_setup(env: &Env, harness: &dyn Harness) -> SkillsSetup {
    match harness.companion_setup().skills {
        SkillsStrategy::Marketplace { source, plugin } => marketplace_skills(env, harness, source, plugin),
        // PI-family settings: a package whose source names the skills' repository.
        SkillsStrategy::Package { source } => {
            if user_has_package(&harness.paths(env, None), SKILLS_PLUGIN) {
                return SkillsSetup::Installed { version: None };
            }

            SkillsSetup::Install {
                actions: vec![ExtensionAction::PackageInstall {
                    source: source.to_owned(),
                }],
            }
        }
        SkillsStrategy::Installer => opencode_skills(env),
        SkillsStrategy::Manual { command, note } => {
            if user_has_package(&harness.paths(env, None), SKILLS_PLUGIN) {
                return SkillsSetup::Installed { version: None };
            }

            SkillsSetup::Manual {
                command: command.to_owned(),
                note: note.to_owned(),
            }
        }
        SkillsStrategy::Unsupported { reason } => SkillsSetup::Unavailable {
            reason: reason.to_owned(),
            code: None,
        },
    }
}

/// Installed when the install record holds a version; otherwise the installer, which needs `node`.
fn opencode_skills(env: &Env) -> SkillsSetup {
    if let Some(version) = opencode_skills_version(env) {
        return SkillsSetup::Installed { version: Some(version) };
    }

    if find_node(env).is_none() {
        return SkillsSetup::Unavailable {
            reason: "Needs Node.js 22 or newer.".to_owned(),
            code: Some(SkillsUnavailableCode::NodeMissing),
        };
    }

    SkillsSetup::Script {
        command: installer(env, Companion::OpenCodeSkills).command.to_owned(),
        side_effects: skills_side_effects(env.os)
            .iter()
            .map(|line| (*line).to_owned())
            .collect(),
    }
}

/// `node` on `PATH` or in the usual install directories, where the skills' installer finds it too
/// (they are appended to its `PATH`). Its version is not checked here, the installer does.
fn find_node(env: &Env) -> Option<ResolvedBinary> {
    resolve_binary(env, "node", None)
}

/// Installed when the user has the skills plugin (`plugin`, or another `name@…` of its name; on OMP
/// also an npm install) and it is switched on; otherwise the marketplace add (unless a marketplace
/// from `source` is registered, see [`same_repository`]) and the user-scope plugin install, or for
/// a plugin that is off its enable.
fn marketplace_skills(env: &Env, harness: &dyn Harness, source: &str, plugin: &str) -> SkillsSetup {
    let name = plugin.split_once('@').map_or(plugin, |(name, _)| name);
    let registries = marketplace_registries(env, harness);
    let prefix = format!("{name}@");

    let installed = registries
        .plugins
        .iter()
        .find(|found| found.id == name || found.id.starts_with(&prefix));

    match installed {
        Some(found) if found.enabled => {
            return SkillsSetup::Installed {
                version: found.version.clone(),
            };
        }
        Some(found) => {
            return SkillsSetup::Install {
                actions: vec![ExtensionAction::PluginEnable { name: found.id.clone() }],
            };
        }
        None => {}
    }

    let registered = registries
        .marketplaces
        .iter()
        .any(|(_, from)| same_repository(from, source));

    let mut actions = Vec::new();

    if !registered {
        actions.push(ExtensionAction::MarketplaceAdd {
            source: source.to_owned(),
        });
    }

    actions.push(ExtensionAction::PluginInstall {
        target: plugin.to_owned(),
        scope: InstallScope::User,
    });

    SkillsSetup::Install { actions }
}

/// What a marketplace harness has installed and registered for its user.
struct MarketplaceRegistries {
    plugins: Vec<FoundPlugin>,
    /// Name and source of each registered marketplace.
    marketplaces: Vec<(String, String)>,
}

/// A plugin installed for the user.
struct FoundPlugin {
    id: String,
    version: Option<String>,
    /// False when the harness's settings do not switch it on.
    enabled: bool,
}

/// Claude Code's plugin and marketplace registries, else OMP's (the user's plugins, every one
/// counted as on, and its marketplace registry).
fn marketplace_registries(env: &Env, harness: &dyn Harness) -> MarketplaceRegistries {
    if harness.id() == HarnessId::Claude {
        return MarketplaceRegistries {
            plugins: claude::user_plugins(env)
                .into_iter()
                .map(|plugin| FoundPlugin {
                    id: plugin.id,
                    version: plugin.version,
                    enabled: plugin.enabled,
                })
                .collect(),
            marketplaces: claude::known_marketplaces(env)
                .into_iter()
                .map(|known| (known.name, known.source))
                .collect(),
        };
    }

    let paths = harness.paths(env, None);
    let plugins = installed_plugins(env, &paths).plugins;
    let (catalogs, _) = marketplaces(env, &paths, &plugins);

    MarketplaceRegistries {
        plugins: plugins
            .into_iter()
            .map(|plugin| FoundPlugin {
                id: plugin.id,
                version: plugin.version,
                enabled: true,
            })
            .collect(),
        marketplaces: catalogs
            .into_iter()
            .map(|catalog| (catalog.name, catalog.source))
            .collect(),
    }
}

/// The app's own server in `file`; a manual edit names this app's program.
fn app_mcp_state(env: &Env, file: &McpFile) -> McpState {
    desktop_mcp_state(env, file, current_app_command().as_deref())
}

/// A server's MCP state in `harness` (`state` reads it from the harness's MCP file and the files it
/// also reads); a harness without an MCP file reports a refusal with no file.
fn harness_mcp_state(env: &Env, harness: &dyn Harness, state: fn(&Env, &McpFile) -> McpState) -> McpState {
    match mcp_target(env, harness) {
        Some(file) => state(env, &file),
        None => McpState::Refused {
            file: String::new(),
            reason: McpRefusal::Unreadable,
        },
    }
}
