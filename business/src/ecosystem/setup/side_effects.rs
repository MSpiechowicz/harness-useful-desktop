//! What each installer the setup step runs changes on this machine, one sentence per line, per
//! its script (`install.sh`, `install.ps1`).

use std::path::Path;

use crate::env::{Env, OsKind};

/// `lines` as owned strings.
pub(super) fn lines(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_owned()).collect()
}

/// What the memory server's installer of `os` changes, per its script (`install.sh`,
/// `install.ps1`).
pub(super) fn memory_side_effects(os: OsKind) -> &'static [&'static str] {
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

/// What the knowledge server's installer of `os` changes, per its script (`install.sh`,
/// `install.ps1`).
pub(super) fn knowledge_side_effects(os: OsKind) -> &'static [&'static str] {
    match os {
        OsKind::Linux | OsKind::MacOs => &[
            "Installs the harness-knowledge command to ~/.local/bin, replacing an older one there.",
            "Appends a line that puts ~/.local/bin on your PATH to your shell profile, if it is not on PATH yet.",
            "Running knowledge servers keep working and each harness starts the new version next time.",
        ],
        OsKind::Windows => &[
            r"Puts the harness-knowledge command in %LOCALAPPDATA%\Programs\harness-knowledge and adds that folder to your user PATH.",
            "Running knowledge servers keep working and each harness starts the new version next time.",
        ],
    }
}

/// What the code server's installer of `os` changes, per its script (`install.sh`, `install.ps1`).
pub(super) fn code_side_effects(os: OsKind) -> &'static [&'static str] {
    match os {
        OsKind::Linux | OsKind::MacOs => &[
            "Installs the harness-code command to ~/.local/bin, replacing an older one there.",
            "Appends a line that puts ~/.local/bin on your PATH to your shell profile, if it is not on PATH yet.",
            "Running code servers keep working and each harness starts the new version next time.",
        ],
        OsKind::Windows => &[
            r"Puts the harness-code command in %LOCALAPPDATA%\Programs\harness-code and adds that folder to your user PATH.",
            "Running code servers keep working and each harness starts the new version next time.",
        ],
    }
}

/// What the installer does on top when it finds an install from before Harness Skills 0.4.0.
pub(super) const MIGRATION_SIDE_EFFECT: &str = "Renames the existing useful-skills install to harness-useful-skills and moves your model roles and settings along.";

/// What the Harness Skills' OpenCode installer of `os` changes, per its script (`install.sh`,
/// `install.ps1`). OpenCode keeps its config folder below the home folder on Windows too.
pub(super) fn skills_side_effects(os: OsKind) -> &'static [&'static str] {
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
pub(super) fn codex_skills_side_effects(env: &Env) -> Vec<String> {
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
pub(super) fn dashboard_side_effects(os: OsKind) -> &'static [&'static str] {
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
