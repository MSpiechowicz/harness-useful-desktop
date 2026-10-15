//! What the "Recommended setup" step offers (`setup_status`): the dashboard binary, and per
//! harness the Harness Skills and the dashboard's MCP server.
//!
//! Everything here only reads: the dashboard's `version` probe, the OMP plugin and marketplace
//! registries, PI's user settings and the user MCP files. The MCP files of a harness are those of
//! its user agent directory ([`Harness::paths`] without a project), so with an OMP profile active
//! only that profile's files count.

use std::path::PathBuf;

use super::catalog::{SKILLS_MARKETPLACE, SKILLS_PLUGIN, SKILLS_TARGET, supported_on};
use super::dashboard::resolve_dashboard;
use super::detect::dashboard_version;
use super::installer::dashboard_installer;
use super::mcp_register::mcp_state;
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
    let dashboard = dashboard_setup(env, runner).await;

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
                mcp: harness_mcp_state(env, harness),
            }
        })
        .collect();

    RecommendedSetup { dashboard, harnesses }
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

async fn dashboard_setup(env: &Env, runner: &dyn ProcessRunner) -> DashboardSetup {
    let binary = resolve_dashboard(env);
    let version = match &binary {
        Some(binary) => dashboard_version(env, runner, binary).await,
        None => None,
    };

    DashboardSetup {
        installed: binary.is_some(),
        version,
        supported_on_this_os: supported_on(EcosystemId::Dashboard, env.os),
        command: dashboard_installer(env).command.to_owned(),
        side_effects: side_effects(env.os).iter().map(|line| (*line).to_owned()).collect(),
    }
}

/// What the installer of `os` changes, per its script (`install.sh`, `install.ps1`).
fn side_effects(os: OsKind) -> &'static [&'static str] {
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

/// The MCP state of `harness`; a harness without a user MCP file reports a refusal with no file.
fn harness_mcp_state(env: &Env, harness: &dyn Harness) -> McpState {
    match mcp_target_paths(env, harness) {
        Some((path, also_read)) => mcp_state(env, &path, &also_read),
        None => McpState::Refused {
            file: String::new(),
            reason: McpRefusal::Unreadable,
        },
    }
}
