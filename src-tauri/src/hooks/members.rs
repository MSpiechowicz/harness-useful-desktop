//! The family members on this machine, and what the hooks' requirements are checked against.

use harness_business::ecosystem::CodeGraphSupport;
use harness_business::hooks::HookCapabilities;

/// The family members on this machine.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Members {
    pub(super) memory_installed: bool,
    pub(super) memory_enabled: bool,
    pub(super) dashboard_installed: bool,
    pub(super) code_graph: CodeGraphSupport,
    pub(super) knowledge_installed: bool,
    pub(super) knowledge_enabled: bool,
    /// The installed Harness Knowledge lists `mcp`.
    pub(super) knowledge_mcp: bool,
}

impl Members {
    /// What the hooks' `requires` are checked against.
    pub(super) fn capabilities(self) -> HookCapabilities {
        HookCapabilities {
            memory_available: self.memory_installed && self.memory_enabled,
            dashboard_available: self.dashboard_installed,
            code_graph_available: self.memory_installed
                && self.memory_enabled
                && self.code_graph.listed
                && self.code_graph.on,
            knowledge_available: self.knowledge_installed && self.knowledge_enabled && self.knowledge_mcp,
        }
    }

    /// Which members are installed (Harness Memory and Harness Knowledge whatever their setting, the
    /// code graph whatever its switch).
    pub(super) fn installed(self) -> HookCapabilities {
        HookCapabilities {
            memory_available: self.memory_installed,
            dashboard_available: self.dashboard_installed,
            code_graph_available: self.memory_installed && self.code_graph.listed,
            knowledge_available: self.knowledge_installed && self.knowledge_mcp,
        }
    }
}
