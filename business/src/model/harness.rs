use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Identifies a supported coding-agent harness.
///
/// Adding a harness means adding a variant here plus a `harnesses/<name>` module that
/// implements the harness trait and a registry entry; the UI renders data-driven from those.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum HarnessId {
    /// oh-my-pi (`omp`).
    Omp,
    /// PI coding agent (`pi`).
    Pi,
    /// OpenCode (`opencode`).
    OpenCode,
    /// Claude Code (`claude`).
    Claude,
}

impl HarnessId {
    /// Every supported harness, in display order.
    pub const ALL: [HarnessId; 4] = [HarnessId::Omp, HarnessId::Pi, HarnessId::OpenCode, HarnessId::Claude];

    /// The default executable name of the harness CLI.
    pub fn binary_name(self) -> &'static str {
        match self {
            HarnessId::Omp => "omp",
            HarnessId::Pi => "pi",
            HarnessId::OpenCode => "opencode",
            HarnessId::Claude => "claude",
        }
    }
}

/// Static description of a harness, shown in onboarding and the harness switcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HarnessInfo {
    /// The harness this describes.
    pub id: HarnessId,
    /// Display name (`"Oh My Pi"`, `"Pi"`).
    pub name: String,
    /// Short label for compact UI (`"OMP"`, `"PI"`).
    pub short_name: String,
    /// Default executable name looked up on PATH (`"omp"`, `"pi"`).
    pub binary_name: String,
    /// One-sentence description of the harness.
    pub description: String,
    /// Project homepage or documentation URL.
    pub homepage: String,
}

/// Feature flags that decide which tabs and actions the UI renders for a harness.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Capabilities {
    /// Chat sessions the app drives (a launch spec and a chat protocol); without it the UI shows
    /// that chat is coming later.
    pub chat: bool,
    /// Installable plugins (OMP `omp plugin …`).
    pub plugins: bool,
    /// Plugin marketplaces with browsable catalogs.
    pub marketplaces: bool,
    /// Packages that bundle resources (PI `pi install …`).
    pub packages: bool,
    /// MCP server configuration.
    pub mcp: bool,
    /// `SKILL.md` skills.
    pub skills: bool,
    /// Markdown prompt templates exposed as slash commands.
    pub prompt_templates: bool,
    /// Named agents the harness can switch to or delegate to (OpenCode `agent`).
    pub agents: bool,
    /// Code extension modules loaded by the harness.
    pub extension_modules: bool,
    /// Custom colour themes.
    pub themes: bool,
    /// Extension actions (install, remove, enable, ...) run through the harness CLI (see
    /// `Harness::cli_action`).
    pub cli_actions: bool,
    /// Named configuration profiles.
    pub profiles: bool,
    /// A configurable tool approval mode (OMP `tools.approvalMode`).
    pub approval_modes: bool,
    /// The approval modes include Yolo (tools run without asking). Without it a harness with
    /// `approval_modes` offers Always ask and Write only (Claude Code: the app never starts its
    /// bypass mode), and a Yolo override is refused.
    pub yolo_mode: bool,
    /// Project trust gating of project-local resources (PI `.pi/`).
    pub project_trust: bool,
    /// Slash commands run by sending their text as a prompt.
    pub slash_commands_via_prompt: bool,
    /// Sessions form a branching tree (fork/resume from any entry).
    pub session_tree: bool,
    /// A running chat can move, conversation included, to another project (OMP `/move`).
    pub move_session: bool,
    /// A message waiting in the steering or follow-up queue can be withdrawn (OMP `remove_queued_message`).
    pub remove_queued: bool,
    /// The chat can switch between the harness's primary agents (OpenCode `build`, `plan`, custom
    /// ones).
    pub agent_picker: bool,
    /// Chat works but is still a preview; the UI marks it with a badge.
    pub chat_preview: bool,
    /// `/compact` takes instructions for the summary (OMP, PI), so picking it asks for them first.
    /// Without it the chat compacts at once.
    pub compact_instructions: bool,
    /// The harness passes `HARNESS_MEMORY_SESSION` on to its MCP servers, so Harness Memory applies
    /// the chat's own guards and can hold a save for review. Without it per-chat rules only skip the
    /// app's memory-save hook, and "Ask before saving" is not offered.
    pub memory_chat_guards: bool,
}

/// How the detected CLI version relates to the versions this app was tested with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum VersionSupport {
    /// Within the tested range.
    Supported,
    /// Newer or older than the tested range; may still work.
    Untested,
    /// The version could not be determined.
    Unknown,
}

/// How to install a missing harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstallHint {
    /// The ways to install it, at least one. A harness shipped as several release lines from
    /// different places (OpenCode 1 and OpenCode 2) has one labelled command per line.
    pub commands: Vec<InstallCommand>,
    /// Installation documentation URL.
    pub docs_url: String,
}

impl InstallHint {
    /// A hint with one unlabelled `command`.
    pub fn single(command: impl Into<String>, docs_url: impl Into<String>) -> Self {
        Self {
            commands: vec![InstallCommand {
                label: None,
                command: command.into(),
            }],
            docs_url: docs_url.into(),
        }
    }
}

/// One copyable install command (shown, never executed by the app).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstallCommand {
    /// What it installs (`"OpenCode 2"`), when the hint has more than one command.
    pub label: Option<String>,
    /// The command line.
    pub command: String,
}

/// Result of probing the machine for a harness CLI and its configuration directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Detection {
    /// True when an executable was found.
    pub installed: bool,
    /// Absolute path of the resolved executable.
    pub binary_path: Option<String>,
    /// True when the executable is a Windows batch shim (`.cmd`/`.bat`).
    pub is_batch_script: bool,
    /// Version reported by the CLI (e.g. `"18.6.1"`).
    pub version: Option<String>,
    /// Whether `version` is in the tested range.
    pub version_support: VersionSupport,
    /// Absolute path of the harness configuration directory (whether or not it exists).
    pub config_dir: Option<String>,
    /// True when `config_dir` exists on disk.
    pub config_dir_exists: bool,
    /// Install instructions, present when the harness is not installed.
    pub install_hint: Option<InstallHint>,
    /// Short, secret-free description of a probe failure (e.g. a version check timeout).
    pub error: Option<String>,
}

/// Everything the UI needs to list a harness: what it is, what it can do and whether it is here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HarnessSummary {
    /// Static description.
    pub info: HarnessInfo,
    /// Feature flags.
    pub capabilities: Capabilities,
    /// Detection result on this machine.
    pub detection: Detection,
    /// The permission mode the harness's own user settings choose, by the harness's name for it
    /// (Claude Code `permissions.defaultMode`); `None` for harnesses without one.
    pub native_approval_mode: Option<NativeApprovalMode>,
}

/// A harness's own permission mode from its settings, as the harness names it. The app's approval
/// modes cover only some of them, so the chat header shows the name read-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NativeApprovalMode {
    /// Claude Code: `default`, `acceptEdits`, `plan`, `auto`, `dontAsk` or `bypassPermissions`.
    pub name: String,
    /// True when the mode lets tools run without asking the user (Claude Code `bypassPermissions`
    /// and `auto`), which onboarding points out.
    pub unattended: bool,
}
