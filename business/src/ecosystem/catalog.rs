//! The companion tools of the ecosystem: names, descriptions, links and copyable install steps.
//!
//! Install steps follow each tool's README (the dashboard's and the memory server's `install.sh` /
//! `install.ps1`, the OMP marketplace installs of the skills and the sidebar). They are shown for
//! copying; the exception is the "Recommended setup" step, which runs the dashboard's, the memory
//! server's or the Harness Skills' OpenCode installer (see [`super::installer`]) after the user
//! confirms it.

use crate::env::OsKind;
use crate::model::{EcosystemId, EcosystemItem, InstallStep};

/// Every tool, in display order.
pub const ECOSYSTEM: [EcosystemId; 4] = [
    EcosystemId::Dashboard,
    EcosystemId::Memory,
    EcosystemId::Skills,
    EcosystemId::Sidebar,
];

/// The dashboard's executable name.
pub const DASHBOARD_BINARY: &str = "harness-dashboard";

/// The npm package / plugin name of the skills.
pub const SKILLS_PLUGIN: &str = "harness-useful-skills";

/// The npm package / plugin name of the sidebar.
pub const SIDEBAR_PLUGIN: &str = "harness-useful-sidebar";

/// The name the setup step gives the dashboard's server in a harness's `mcpServers`.
pub const DASHBOARD_MCP_SERVER: &str = "harness-dashboard";

/// The project memory server's executable name (Harness Memory, installed like the dashboard).
pub const MEMORY_BINARY: &str = "harness-memory";

/// The name the app gives the memory server in a harness's `mcpServers`.
pub const MEMORY_MCP_SERVER: &str = "harness-memory";

/// The name the skills' installers give their MCP server (Harness Skills 0.4.0 and newer): in
/// OpenCode's `mcp` (a `local` server running the installed `mcp-server.js` by absolute path), in
/// Codex's `mcp_servers`, and the one the Claude Code and OMP plugins declare.
pub const SKILLS_MCP_SERVER: &str = "harness-useful-skills";

/// The same server before Harness Skills 0.4.0 unified the names. Installs older than that still
/// use it until the user runs the installer again, so it stays recognised (and reserved).
pub const LEGACY_SKILLS_MCP_SERVER: &str = "useful_skills";

/// The folder Harness Skills 0.4.0 and newer keep their data in, below Claude Code's config
/// directory (`models.yml`, `workflow-settings`), below OpenCode's `plugins` and config directory,
/// and below the Codex home (`install-record.json`, `models.yml`).
pub const SKILLS_DIR: &str = "harness-useful-skills";

/// The folder of the same data before Harness Skills 0.4.0.
pub const LEGACY_SKILLS_DIR: &str = "useful-skills";

/// The plugin name before Harness Skills 0.4.0 (`useful-skills@useful-skills-local` in Claude
/// Code, `useful-skills@harness-useful-skills` in OMP). An install under it is recognised and
/// offered the update that renames it.
pub const LEGACY_SKILLS_PLUGIN: &str = "useful-skills";

/// The first Harness Skills release with the unified names ([`SKILLS_DIR`], [`SKILLS_MCP_SERVER`],
/// [`SKILLS_TARGET`]) and, for Codex, model roles.
pub const SKILLS_UNIFIED_VERSION: &str = "0.4.0";

/// The skills' marketplace source (`omp plugin marketplace add …`, `claude plugin marketplace add
/// …`).
pub const SKILLS_MARKETPLACE: &str = "MSpiechowicz/harness-useful-skills";

/// The skills' OMP plugin target (`omp plugin install …`).
pub const SKILLS_TARGET: &str = "harness-useful-skills@harness-useful-skills";

/// The skills' Claude Code plugin (`claude plugin install … --scope user`): the marketplace of
/// [`SKILLS_MARKETPLACE`] names itself `harness-useful-skills` since Harness Skills 0.4.0 (it was
/// `useful-skills-local`), so the ID is the OMP one.
pub const CLAUDE_SKILLS_PLUGIN: &str = SKILLS_TARGET;

/// The skills as a PI package source (`pi install …`).
pub const PI_SKILLS_SOURCE: &str = "git:github.com/MSpiechowicz/harness-useful-skills";

/// The dashboard's Unix installer, as a copyable shell command (also the script the setup step
/// runs on Linux and macOS). `curl` is held to HTTPS and TLS 1.2 or newer, redirects included.
pub const DASHBOARD_INSTALL_SH: &str = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-dashboard/main/install.sh | sh";

/// The PowerShell script of the dashboard's Windows installer (what the setup step runs there).
pub const DASHBOARD_INSTALL_PS1_SCRIPT: &str =
    "irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-dashboard/main/install.ps1 | iex";

/// The dashboard's Windows installer, as a copyable command.
pub const DASHBOARD_INSTALL_PS1: &str = "powershell -ExecutionPolicy Bypass -c \"irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-dashboard/main/install.ps1 | iex\"";

/// The memory server's Unix installer, as a copyable shell command (also the script the setup
/// step runs on Linux and macOS). `curl` is held to HTTPS and TLS 1.2 or newer, redirects included.
pub const MEMORY_INSTALL_SH: &str = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-memory/main/install.sh | sh";

/// The PowerShell script of the memory server's Windows installer (what the setup step runs there).
pub const MEMORY_INSTALL_PS1_SCRIPT: &str =
    "irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-memory/main/install.ps1 | iex";

/// The memory server's Windows installer, as a copyable command.
pub const MEMORY_INSTALL_PS1: &str = "powershell -ExecutionPolicy Bypass -c \"irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-memory/main/install.ps1 | iex\"";

/// The Harness Skills' Unix installer for OpenCode (the script's default host), as a copyable shell command (also the script
/// the setup step runs on Linux and macOS). `curl` is held to HTTPS and TLS 1.2 or newer, redirects
/// included.
pub const SKILLS_INSTALL_SH: &str = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.sh | sh";

/// The PowerShell script of the Harness Skills' Windows installer for OpenCode (what the setup
/// step runs there).
pub const SKILLS_INSTALL_PS1_SCRIPT: &str =
    "irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.ps1 | iex";

/// The Harness Skills' Windows installer for OpenCode, as a copyable command.
pub const SKILLS_INSTALL_PS1: &str = "powershell -ExecutionPolicy Bypass -c \"irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.ps1 | iex\"";

/// The Harness Skills' Unix installer for Codex (the same script with `HARNESS_SKILLS_HOST=codex`,
/// Harness Skills 0.3.13 and newer), as a copyable shell command (also the script the setup step
/// runs on Linux and macOS).
pub const CODEX_SKILLS_INSTALL_SH: &str = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.sh | HARNESS_SKILLS_HOST=codex sh";

/// The PowerShell script of the Harness Skills' Windows installer for Codex (what the setup step
/// runs there), also its copyable command: it is meant to be pasted into a PowerShell prompt.
pub const CODEX_SKILLS_INSTALL_PS1: &str = "$env:HARNESS_SKILLS_HOST = 'codex'; irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.ps1 | iex";

/// The first Harness Skills release with a Codex adapter: the installer refuses older ones, and
/// an older install record counts as needing an update.
pub const CODEX_SKILLS_MIN_VERSION: &str = "0.3.13";

/// True when `id` runs on `os` (the sidebar needs tmux or a native pane: Linux and macOS only).
pub fn supported_on(id: EcosystemId, os: OsKind) -> bool {
    id != EcosystemId::Sidebar || os != OsKind::Windows
}

/// The catalog entry of `id` for `os`, not installed (detection fills `installed`, `version` and
/// `via`).
pub fn catalog_item(id: EcosystemId, os: OsKind) -> EcosystemItem {
    let supported = supported_on(id, os);
    let mut item = match id {
        EcosystemId::Dashboard => dashboard(os),
        EcosystemId::Memory => memory(os),
        EcosystemId::Skills => skills(),
        EcosystemId::Sidebar => sidebar(),
    };

    item.supported_on_this_os = supported;
    if !supported {
        item.install_steps.clear();
    }

    item
}

fn dashboard(os: OsKind) -> EcosystemItem {
    let install = match os {
        OsKind::Windows => DASHBOARD_INSTALL_PS1,
        OsKind::Linux | OsKind::MacOs => DASHBOARD_INSTALL_SH,
    };

    item(
        EcosystemId::Dashboard,
        "Harness Dashboard",
        "Token usage across your AI coding tools, in one local app.",
        "Reads the sessions Claude Code, Codex, OMP, PI and other agents write to disk and shows cost, tokens, \
         plan limits and budgets. The app reads it through its local, read-only MCP server.",
        vec![step("Install the binary", install)],
        &[
            "Data comes from the local process `harness-dashboard mcp`; nothing is sent over the network.",
            "Run `harness-dashboard` now and then so its database stays current.",
        ],
    )
}

fn memory(os: OsKind) -> EcosystemItem {
    let install = match os {
        OsKind::Windows => MEMORY_INSTALL_PS1,
        OsKind::Linux | OsKind::MacOs => MEMORY_INSTALL_SH,
    };

    item(
        EcosystemId::Memory,
        "Harness Memory",
        "One project memory for every coding agent, kept on this machine.",
        "A local MCP server that keeps notes and how they relate per project, so what one agent learns the \
         next one finds. The app registers it with each detected harness while Project memory is on, and adds \
         matching notes to the first message of a new chat.",
        vec![step("Install the binary", install)],
        &[
            "Runs locally as `harness-memory mcp`; nothing is sent over the network.",
            "Turn it on or off for every harness in Settings, under Project memory.",
        ],
    )
}

fn skills() -> EcosystemItem {
    item(
        EcosystemId::Skills,
        "Harness Skills",
        "From a natural-language request to reviewed, verified work.",
        "Owned `us-*` skills for development, focused audits, delivery and project memory, installed as an OMP \
         plugin.",
        marketplace_steps(SKILLS_PLUGIN),
        &[
            "Requires Node.js 22 or newer.",
            "Restart OMP after installing or updating.",
        ],
    )
}

fn sidebar() -> EcosystemItem {
    let mut steps = marketplace_steps(SIDEBAR_PLUGIN);
    steps.push(step(
        "Set up the sidebar",
        "python3 ~/.omp/plugins/node_modules/harness-useful-sidebar/install.py --native",
    ));

    item(
        EcosystemId::Sidebar,
        "Harness Sidebar",
        "Live token charts and persistent session history for OMP in a tmux sidebar.",
        "Shows live token charts and the session history next to OMP in tmux, and in a native pane for Claude \
         Code.",
        steps,
        &["Linux and macOS only.", "Needs tmux for the OMP sidebar."],
    )
}

/// Adding the tool's own marketplace, then installing its plugin for the user.
fn marketplace_steps(plugin: &str) -> Vec<InstallStep> {
    vec![
        step(
            "Add the marketplace",
            &format!("omp plugin marketplace add MSpiechowicz/{plugin}"),
        ),
        step(
            "Install the plugin",
            &format!("omp plugin install {plugin}@{plugin} --scope user"),
        ),
    ]
}

fn item(
    id: EcosystemId,
    name: &str,
    tagline: &str,
    description: &str,
    install_steps: Vec<InstallStep>,
    notes: &[&str],
) -> EcosystemItem {
    let repo = repo_url(id);

    EcosystemItem {
        id,
        name: name.to_owned(),
        tagline: tagline.to_owned(),
        description: description.to_owned(),
        docs_url: format!("{repo}#readme"),
        repo_url: repo,
        installed: false,
        version: None,
        latest_version: None,
        update_available: false,
        via: Vec::new(),
        supported_on_this_os: true,
        install_steps,
        notes: notes.iter().map(|note| (*note).to_owned()).collect(),
    }
}

fn repo_url(id: EcosystemId) -> String {
    let name = match id {
        EcosystemId::Dashboard => "harness-useful-dashboard",
        EcosystemId::Memory => "harness-useful-memory",
        EcosystemId::Skills => SKILLS_PLUGIN,
        EcosystemId::Sidebar => SIDEBAR_PLUGIN,
    };

    format!("https://github.com/MSpiechowicz/{name}")
}

fn step(label: &str, command: &str) -> InstallStep {
    InstallStep {
        label: label.to_owned(),
        command: command.to_owned(),
    }
}
