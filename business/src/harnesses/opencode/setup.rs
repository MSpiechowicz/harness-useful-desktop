//! How the "Recommended setup" step provides the companion tools for OpenCode.

use crate::harness::{CompanionSetup, McpTarget, OpenCodeGeneration, SkillsHost, SkillsStrategy};

/// The Harness Skills come from their own installer script; the MCP servers go into the global
/// config file of `generation`.
pub fn companion_setup(generation: OpenCodeGeneration) -> CompanionSetup {
    CompanionSetup {
        skills: SkillsStrategy::Installer(SkillsHost::OpenCode),
        mcp: McpTarget::OpenCodeConfig(generation),
    }
}
