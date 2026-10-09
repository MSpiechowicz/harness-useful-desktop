//! The companion tools of the ecosystem: names, descriptions, links and copyable install steps.
//!
//! Install steps follow each tool's README (the dashboard's `install.sh` / `install.ps1`, the
//! OMP marketplace installs of the skills and the sidebar). They are shown for copying and are
//! never run by the app.

use crate::env::OsKind;
use crate::model::{EcosystemId, EcosystemItem, InstallStep};

/// Every tool, in display order.
pub const ECOSYSTEM: [EcosystemId; 3] = [EcosystemId::Dashboard, EcosystemId::Skills, EcosystemId::Sidebar];

/// The dashboard's executable name.
pub const DASHBOARD_BINARY: &str = "harness-dashboard";

/// The npm package / plugin name of the skills.
pub const SKILLS_PLUGIN: &str = "harness-useful-skills";

/// The npm package / plugin name of the sidebar.
pub const SIDEBAR_PLUGIN: &str = "harness-useful-sidebar";

const DASHBOARD_INSTALL_SH: &str =
    "curl -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-dashboard/main/install.sh | sh";

const DASHBOARD_INSTALL_PS1: &str = "powershell -ExecutionPolicy Bypass -c \"irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-dashboard/main/install.ps1 | iex\"";

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

fn skills() -> EcosystemItem {
    item(
        EcosystemId::Skills,
        "Useful Skills",
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
        "Useful Sidebar",
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
        via: Vec::new(),
        supported_on_this_os: true,
        install_steps,
        notes: notes.iter().map(|note| (*note).to_owned()).collect(),
    }
}

fn repo_url(id: EcosystemId) -> String {
    let name = match id {
        EcosystemId::Dashboard => "harness-useful-dashboard",
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
