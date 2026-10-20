//! What the "Recommended setup" step offers (`setup_status`): the dashboard and memory binaries,
//! and per harness the Harness Skills and the dashboard's, the memory server's and the app's own
//! (`harness-desktop mcp`) MCP servers.
//!
//! Everything here only reads: the dashboard's and the memory server's version probes, the OMP plugin and marketplace
//! registries, PI's user settings, the Harness Skills' OpenCode install record (see
//! [`super::skills_record`]), where `node` is (their installer needs it) and the user MCP files. The MCP files of a harness are those of
//! its user agent directory ([`Harness::paths`] without a project), so with an OMP profile active
//! only that profile's files count. OpenCode's is its global config file ([`mcp_target`]).

use std::fs;
use std::path::PathBuf;

use super::catalog::{SKILLS_MARKETPLACE, SKILLS_PLUGIN, SKILLS_TARGET, supported_on};
use super::desktop_register::{current_app_command, desktop_mcp_state};
use super::installer::{Companion, Installed, installer};
use super::mcp_merge::Shape;
use super::mcp_register::{mcp_state, memory_mcp_state};
use super::skills_record::opencode_skills_version;
use crate::env::{Env, OsKind};
use crate::harness::{Harness, HarnessRegistry, McpTarget, SkillsStrategy};
use crate::harnesses::omp::{installed_plugins, marketplaces};
use crate::harnesses::opencode;
use crate::harnesses::pi::user_has_package;
use crate::model::{
    DashboardSetup, EcosystemId, ExtensionAction, HarnessSetup, HarnessSummary, InstallScope, McpRefusal, McpState,
    RecommendedSetup, SkillsSetup,
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
    /// The file written.
    pub path: PathBuf,
    /// Files that can define servers too; only read, to find an existing entry.
    pub also_read: Vec<PathBuf>,
    /// How the files hold their servers.
    pub shape: Shape,
}

/// The file the setup step and project memory register servers in for `harness`; `None` when the
/// harness has none.
///
/// OpenCode merges every global config file it finds. The app writes the one OpenCode itself
/// would edit (`opencode.jsonc`, else `opencode.json`, in the config directory), creating
/// `opencode.json` when there is neither, and only reads the others: `config.json` there,
/// `opencode.json[c]` in `~/.opencode` and in `OPENCODE_CONFIG_DIR`, and the file `OPENCODE_CONFIG`
/// names.
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
        McpTarget::OpenCodeConfig => Some(opencode_file(env)),
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
fn opencode_file(env: &Env) -> McpFile {
    let config = opencode::config_dir(env);
    let jsonc = config.join("opencode.jsonc");
    let json = config.join("opencode.json");

    // A symlink counts as there, so it is refused rather than passed over.
    let path = if fs::symlink_metadata(&jsonc).is_ok() {
        jsonc
    } else {
        json
    };

    let extra_dirs = std::iter::once(opencode::home_config_dir(env)).chain(opencode::custom_config_dir(env));
    let candidates = ["config.json", "opencode.json", "opencode.jsonc"]
        .map(|name| config.join(name))
        .into_iter()
        .chain(extra_dirs.flat_map(|dir| [dir.join("opencode.json"), dir.join("opencode.jsonc")]))
        .chain(env_config_file(env));

    let mut also_read: Vec<PathBuf> = Vec::new();
    for candidate in candidates {
        if candidate != path && !also_read.contains(&candidate) {
            also_read.push(candidate);
        }
    }

    McpFile {
        path,
        also_read,
        shape: Shape::OpenCode,
    }
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
        SkillsStrategy::OmpMarketplace => omp_skills(env, harness),
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

/// Installed when the user has the skills plugin (marketplace or npm install); otherwise the
/// marketplace add (unless it is registered) and the user-scope plugin install.
fn omp_skills(env: &Env, harness: &dyn Harness) -> SkillsSetup {
    let paths = harness.paths(env, None);
    let plugins = installed_plugins(env, &paths).plugins;
    let marketplace_prefix = format!("{SKILLS_PLUGIN}@");

    let installed = plugins
        .iter()
        .find(|plugin| plugin.id == SKILLS_PLUGIN || plugin.id.starts_with(&marketplace_prefix));

    if let Some(plugin) = installed {
        return SkillsSetup::Installed {
            version: plugin.version.clone(),
        };
    }

    let (catalogs, _) = marketplaces(env, &paths, &plugins);
    let marketplace_name = SKILLS_TARGET.rsplit('@').next().unwrap_or(SKILLS_PLUGIN);
    let registered = catalogs.iter().any(|catalog| {
        catalog.name == marketplace_name
            || catalog
                .source
                .to_ascii_lowercase()
                .contains(&SKILLS_MARKETPLACE.to_ascii_lowercase())
    });

    let mut actions = Vec::new();

    if !registered {
        actions.push(ExtensionAction::MarketplaceAdd {
            source: SKILLS_MARKETPLACE.to_owned(),
        });
    }

    actions.push(ExtensionAction::PluginInstall {
        target: SKILLS_TARGET.to_owned(),
        scope: InstallScope::User,
    });

    SkillsSetup::Install { actions }
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
