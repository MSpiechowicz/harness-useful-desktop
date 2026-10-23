//! How the "Recommended setup" step provides the companion tools (the Harness Skills and the
//! companion MCP servers) for a harness ([`super::Harness::companion_setup`]).

use super::OpenCodeGeneration;

/// How the "Recommended setup" step provides the companion tools for a harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompanionSetup {
    /// How the Harness Skills are installed.
    pub skills: SkillsStrategy,
    /// Where the `harness-dashboard` MCP server is registered.
    pub mcp: McpTarget,
}

/// How the Harness Skills get into a harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillsStrategy {
    /// A plugin marketplace of the harness's CLI (OMP's `omp plugin`, Claude Code's `claude
    /// plugin`): add the marketplace unless it is registered, then install the plugin for the user.
    Marketplace {
        /// The marketplace source, as the CLI's `plugin marketplace add` takes it (`owner/repo`).
        source: &'static str,
        /// The plugin to install, `name@marketplace`.
        plugin: &'static str,
    },
    /// A package of the harness's own package manager (PI's `pi install`): the app installs it for the user.
    Package {
        /// The package source, as the harness CLI takes it (for example `git:github.com/owner/repo`).
        source: &'static str,
    },
    /// The Harness Skills' own installer script (`install.sh` / `install.ps1`, OpenCode and Codex):
    /// the app runs it for the user once they confirm, and its install record tells whether it is
    /// there.
    Installer(SkillsHost),
    /// A command the user runs themselves.
    Manual {
        /// The command to copy.
        command: &'static str,
        /// A remark shown next to it.
        note: &'static str,
    },
    /// The skills do not exist for this harness.
    Unsupported {
        /// Why, for display.
        reason: &'static str,
    },
}

/// The harness an installer script of the Harness Skills installs for (its `HARNESS_SKILLS_HOST`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillsHost {
    /// OpenCode, the script's default.
    OpenCode,
    /// Codex (`HARNESS_SKILLS_HOST=codex`, Harness Skills 0.3.13 and newer).
    Codex,
}

/// Where a harness reads user MCP servers from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTarget {
    /// `<agent dir>/mcp.json` with an `mcpServers` object (written by the setup step), plus other
    /// files in the agent directory that define servers too (only read, to find an existing entry).
    UserMcpJson {
        /// File names in the agent directory that are also read.
        also_read: &'static [&'static str],
    },
    /// The servers of the global OpenCode config file of this generation, whose entries run
    /// `command` arrays: `mcp.<name>` on OpenCode 1, `mcp.servers.<name>` on OpenCode 2 (see
    /// [`crate::ecosystem::mcp_target`]).
    OpenCodeConfig(OpenCodeGeneration),
    /// Claude Code's user scope: the top-level `mcpServers` of its global file (`~/.claude.json`),
    /// which the app only reads. Servers are added and removed through `claude mcp add-json
    /// --scope user` and `claude mcp remove --scope user` (see [`crate::ecosystem::mcp_cli`]).
    ClaudeCli,
    /// Codex's user settings: the `mcp_servers` table of its `config.toml`, which the app only
    /// reads. Servers are added and removed through `codex mcp add` and `codex mcp remove`, which
    /// keep the file's formatting and comments (see [`crate::ecosystem::mcp_cli`]).
    CodexCli,
    /// The harness has no user MCP file the app can register a server in.
    Unsupported,
}
