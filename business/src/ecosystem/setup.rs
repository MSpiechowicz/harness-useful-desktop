//! What the "Recommended setup" step offers (`setup_status`): the dashboard and memory binaries,
//! and per harness the Harness Skills and the dashboard's and the memory server's MCP servers.
//!
//! Everything here only reads: the dashboard's and the memory server's version probes, the OMP plugin and marketplace
//! registries, PI's user settings and the user MCP files. The MCP files of a harness are those of
//! its user agent directory ([`Harness::paths`] without a project), so with an OMP profile active
//! only that profile's files count.

use std::path::{Path, PathBuf};

use super::catalog::{SKILLS_MARKETPLACE, SKILLS_PLUGIN, SKILLS_TARGET, supported_on};
use super::installer::{Companion, installer};
use super::mcp_register::{mcp_state, memory_mcp_state};
use crate::env::{Env, OsKind};
use crate::harness::{Harness, HarnessRegistry, McpTarget, SkillsStrategy};
use crate::harnesses::omp::{installed_plugins, marketplaces};
use crate::harnesses::pi::user_has_package;
use crate::model::{
    DashboardSetup, EcosystemId, ExtensionAction, HarnessSetup, HarnessSummary, InstallScope, McpRefusal, McpState,
    RecommendedSetup, SkillsSetup,
};
use crate::process::ProcessRunner;

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
            }
        })
        .collect();

    RecommendedSetup {
        dashboard,
        memory,
        harnesses,
    }
}

/// The user MCP file the setup step writes for `harness`, and the files it also reads; `None` when
/// the harness has none.
pub fn mcp_target_paths(env: &Env, harness: &dyn Harness) -> Option<(PathBuf, Vec<PathBuf>)> {
    let McpTarget::UserMcpJson { also_read } = harness.companion_setup().mcp else {
        return None;
    };

    let agent_dir = harness.paths(env, None).agent_dir;
    let others = also_read.iter().map(|name| agent_dir.join(name)).collect();

    Some((agent_dir.join("mcp.json"), others))
}

/// The `harness-memory` binary: whether and which version is installed, and its installer.
pub async fn memory_server_setup(env: &Env, runner: &dyn ProcessRunner) -> DashboardSetup {
    binary_setup(env, runner, Companion::Memory).await
}

async fn binary_setup(env: &Env, runner: &dyn ProcessRunner, companion: Companion) -> DashboardSetup {
    let binary = companion.resolve(env);
    let version = match &binary {
        Some(binary) => companion.version(env, runner, binary).await,
        None => None,
    };

    let (id, side_effects) = match companion {
        Companion::Dashboard => (EcosystemId::Dashboard, dashboard_side_effects(env.os)),
        Companion::Memory => (EcosystemId::Memory, memory_side_effects(env.os)),
    };

    // A memory server without a version is never registered or run, so the installer is offered.
    let installed = match companion {
        Companion::Dashboard => binary.is_some(),
        Companion::Memory => version.is_some(),
    };

    DashboardSetup {
        installed,
        version,
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

/// A server's MCP state in `harness` (`state` reads it from the user MCP file and the files it also
/// reads); a harness without a user MCP file reports a refusal with no file.
fn harness_mcp_state(env: &Env, harness: &dyn Harness, state: fn(&Env, &Path, &[PathBuf]) -> McpState) -> McpState {
    match mcp_target_paths(env, harness) {
        Some((path, also_read)) => state(env, &path, &also_read),
        None => McpState::Refused {
            file: String::new(),
            reason: McpRefusal::Unreadable,
        },
    }
}
