//! What the "Recommended setup" step offers (`setup_status`): the dashboard and memory binaries,
//! and per harness the Harness Skills and the dashboard's, the memory server's and the app's own
//! (`harness-desktop mcp`) MCP servers.
//!
//! Everything here only reads: the dashboard's and the memory server's version probes, the OMP,
//! Claude Code and Codex plugin and marketplace registries ([`registries`]), PI's user settings, the Harness Skills' OpenCode and Codex install records (see
//! [`super::skills_record`]), where `node` is (their installer needs it) and the user MCP files. The MCP files of a harness are those of
//! its user agent directory ([`Harness::paths`] without a project), so with an OMP profile active
//! only that profile's files count. OpenCode's is its global config file, Claude Code's the
//! `mcpServers` of its global file, Codex's the `mcp_servers` of its `config.toml` ([`mcp_target`]).

mod github;
mod legacy;
mod registries;

use std::path::{Path, PathBuf};

use self::github::same_repository;
pub use self::legacy::{LegacySkillsStep, legacy_skills_step};
use self::legacy::{install_actions, is_plugin_named, legacy_plugin_name};
use self::registries::marketplace_registries;
use super::catalog::{CODEX_SKILLS_MIN_VERSION, SKILLS_PLUGIN, supported_on};
use super::desktop_register::{current_app_command, desktop_mcp_state};
use super::installer::{Companion, Installed, installer};
use super::mcp_merge::Shape;
use super::mcp_register::{mcp_state, memory_mcp_state};
use super::releases::is_newer_release;
use super::skills_record::skills_install;
use crate::env::{Env, OsKind};
use crate::harness::{Harness, HarnessRegistry, McpTarget, OpenCodeGeneration, SkillsHost, SkillsStrategy};
use crate::harnesses::pi::user_has_package;
use crate::harnesses::{claude, codex, opencode};
use crate::model::{
    DashboardSetup, EcosystemId, ExtensionAction, HarnessSetup, HarnessSummary, McpRefusal, McpState, RecommendedSetup,
    SkillsSetup, SkillsUnavailableCode,
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
    /// The file written (only read for a [`Shape::ClaudeCli`] or [`Shape::CodexCli`] file, which
    /// the harness's CLI changes, see [`McpFile::via_cli`]).
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
        McpTarget::CodexCli => Some(McpFile {
            path: codex::user_config_path(env),
            also_read: Vec::new(),
            shape: Shape::CodexCli,
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
        Companion::Dashboard => (EcosystemId::Dashboard, lines(dashboard_side_effects(env.os))),
        Companion::Memory => (EcosystemId::Memory, lines(memory_side_effects(env.os))),
        Companion::OpenCodeSkills => (EcosystemId::Skills, lines(skills_side_effects(env.os))),
        Companion::CodexSkills => (EcosystemId::Skills, codex_skills_side_effects(env)),
    };

    // A memory server without a version is never registered or run, so the installer is offered.
    let installed = match companion {
        Companion::Dashboard => found != Installed::Missing,
        Companion::Memory | Companion::OpenCodeSkills | Companion::CodexSkills => found.version().is_some(),
    };

    DashboardSetup {
        installed,
        version: found.version().map(str::to_owned),
        supported_on_this_os: supported_on(id, env.os),
        command: installer(env, companion).command.to_owned(),
        side_effects,
    }
}

/// `lines` as owned strings.
fn lines(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_owned()).collect()
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

/// What the installer does on top when it finds an install from before Harness Skills 0.4.0.
const MIGRATION_SIDE_EFFECT: &str = "Renames the existing useful-skills install to harness-useful-skills and moves your model roles and settings along.";

/// What the Harness Skills' OpenCode installer of `os` changes, per its script (`install.sh`,
/// `install.ps1`). OpenCode keeps its config folder below the home folder on Windows too.
fn skills_side_effects(os: OsKind) -> &'static [&'static str] {
    match os {
        OsKind::Linux | OsKind::MacOs => &[
            "Installs the Harness Skills plugin, skills, agents and commands into the OpenCode config folder (~/.config/opencode).",
            "Adds the harness-useful-skills MCP server to opencode.json in that folder.",
            "Needs Node.js 22 or newer.",
            "Restart OpenCode and running OpenCode chats to load the skills.",
            "Updates come from /useful-skills update in OpenCode.",
        ],
        OsKind::Windows => &[
            r"Installs the Harness Skills plugin, skills, agents and commands into the OpenCode config folder (%USERPROFILE%\.config\opencode).",
            "Adds the harness-useful-skills MCP server to opencode.json in that folder.",
            "Needs Node.js 22 or newer.",
            "Restart OpenCode and running OpenCode chats to load the skills.",
            "Updates come from /useful-skills update in OpenCode.",
        ],
    }
}

/// What the Harness Skills' Codex installer changes, per its script (`install.sh`, `install.ps1`).
/// The Codex folder is the absolute `CODEX_HOME`, else the default one below the home folder.
fn codex_skills_side_effects(env: &Env) -> Vec<String> {
    let folder = match env.var("CODEX_HOME").filter(|home| Path::new(home).is_absolute()) {
        Some(home) => home.to_owned(),
        None if env.os == OsKind::Windows => r"%USERPROFILE%\.codex".to_owned(),
        None => "~/.codex".to_owned(),
    };

    vec![
        "Adds the Harness Skills marketplace and plugin to Codex.".to_owned(),
        format!("Writes the us-* agents and the memory runtime into the Codex folder ({folder})."),
        "Registers the harness-useful-skills MCP server with codex mcp add.".to_owned(),
        "Needs Node.js 22 or newer and the codex command.".to_owned(),
        "Restart Codex and running Codex chats to load the skills.".to_owned(),
        "Updates come from running this installer again.".to_owned(),
    ]
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
        SkillsStrategy::Installer(host) => installer_skills(env, host),
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

/// Installed when the install record holds a version that `host` supports (see
/// [`unsupported_version`]). Otherwise the installer, which needs `node`, and for an older record
/// the version it is updated from.
fn installer_skills(env: &Env, host: SkillsHost) -> SkillsSetup {
    let record = skills_install(env, host);
    let version = record.installed.version().map(str::to_owned);

    // An install under the names before 0.4.0 works, but only the installer renames it.
    if let Some(version) = &version
        && !record.legacy
        && !unsupported_version(host, version)
    {
        return SkillsSetup::Installed {
            version: Some(version.clone()),
        };
    }

    if find_node(env).is_none() {
        return SkillsSetup::Unavailable {
            reason: "Needs Node.js 22 or newer.".to_owned(),
            code: Some(SkillsUnavailableCode::NodeMissing),
        };
    }

    let (companion, mut side_effects) = match host {
        SkillsHost::OpenCode => (Companion::OpenCodeSkills, lines(skills_side_effects(env.os))),
        SkillsHost::Codex => (Companion::CodexSkills, codex_skills_side_effects(env)),
    };

    if record.legacy {
        side_effects.push(MIGRATION_SIDE_EFFECT.to_owned());
    }

    SkillsSetup::Script {
        command: installer(env, companion).command.to_owned(),
        side_effects,
        outdated_from: version,
    }
}

/// True for a record version older than the first Harness Skills release that supports `host`:
/// OpenCode's installer has always been there, Codex's starts with [`CODEX_SKILLS_MIN_VERSION`].
fn unsupported_version(host: SkillsHost, version: &str) -> bool {
    match host {
        SkillsHost::OpenCode => false,
        SkillsHost::Codex => is_newer_release(CODEX_SKILLS_MIN_VERSION, version),
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

    // The skills plugin under its name before 0.4.0 still counts, the new name first.
    let installed = registries
        .plugins
        .iter()
        .find(|found| is_plugin_named(&found.id, name))
        .or_else(|| {
            let legacy = legacy_plugin_name(name)?;

            registries
                .plugins
                .iter()
                .find(|found| is_plugin_named(&found.id, legacy))
        });

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

    SkillsSetup::Install {
        actions: install_actions(registered, source, plugin),
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
