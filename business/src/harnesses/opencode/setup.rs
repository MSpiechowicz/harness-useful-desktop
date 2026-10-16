//! How the "Recommended setup" step provides the companion tools for OpenCode.

use crate::harness::{CompanionSetup, McpTarget, SkillsStrategy};

/// The Harness Skills have no OpenCode package; the MCP servers go into the global config file.
pub fn companion_setup() -> CompanionSetup {
    CompanionSetup {
        skills: SkillsStrategy::Unsupported {
            reason: "Harness Skills has no OpenCode package yet.",
        },
        mcp: McpTarget::OpenCodeConfig,
    }
}
