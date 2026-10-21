//! How the "Recommended setup" step provides the companion tools for OpenCode.

use crate::harness::{CompanionSetup, McpTarget, OpenCodeGeneration, SkillsStrategy};

/// The Harness Skills come from their own installer script; the MCP servers go into the global
/// config file of `generation`.
pub fn companion_setup(generation: OpenCodeGeneration) -> CompanionSetup {
    CompanionSetup {
        skills: SkillsStrategy::Installer,
        mcp: McpTarget::OpenCodeConfig(generation),
    }
}
