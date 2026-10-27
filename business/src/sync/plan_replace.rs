//! The rows of an MCP server that exists here with another definition ([`SyncItemStatus::
//! DifferentDefinition`]): what the review shows for replacing it and what Rust keeps to do so. See
//! [`super::plan`].
//!
//! For an HTTP or SSE server at the same endpoint a replace sets only the managed fields of the existing entry (see
//! [`crate::ecosystem::Shape::managed_keys`]): its other keys, an on/off switch included, keep their
//! values, so the step shown never carries `enabled` or `disabled`. For anything else (a stdio server, another endpoint, another
//! transport) the whole entry is replaced by what a new add writes, and the step shown is that add's (see
//! [`super::endpoint`]). Nothing read from this machine is kept: the existing entry is read again
//! when the replace runs.

use super::plan_presence::McpFiles;
use super::plan_rows::{Row, Step, mcp_definition};
use super::schema::McpServer;
use crate::ecosystem::Shape;
use crate::ecosystem::mcp_cli::{add_args, remove_args};
use crate::model::{HarnessId, SyncMcpPreview, SyncStep};

impl Row {
    /// Makes a `differentDefinition` MCP row selectable (never ticked by default) with the
    /// displayed `steps`; `command` is the display line of the add (CLI shapes).
    pub fn select_replace(&mut self, step: Step, steps: Vec<SyncStep>, command: Vec<String>, files: McpFiles) {
        self.item.selectable = true;
        self.item.selected_by_default = false;
        self.item.steps = steps;
        self.stored.step = Some(step);
        self.stored.command = command;
        self.stored.mcp_files = Some(files);
        self.stored.replace = true;
    }
}

/// The step shown for replacing `server` in `file` (a home-relative display path): the managed
/// fields only, `enabled` is always true and not written.
pub(super) fn mcp_replace_preview(file: String, server: &McpServer) -> SyncStep {
    SyncStep::McpReplace {
        file,
        server: SyncMcpPreview {
            name: server.name.clone(),
            transport: server.transport.into(),
            command: server.command.clone(),
            args: server.args.clone(),
            url: server.url.clone(),
            enabled: true,
        },
    }
}

/// The two command lines shown for replacing `server` of `harness` through its CLI: the removal,
/// then the add with the bundle's definition (the old entry's other settings are carried over when
/// it runs, and never shown).
pub(super) fn mcp_cli_replace_preview(harness: HarnessId, server: &McpServer, shape: Shape) -> Vec<SyncStep> {
    let line = |args: Option<Vec<String>>| {
        let mut command = vec![harness.binary_name().to_owned()];
        command.extend(args.unwrap_or_default());

        SyncStep::Cli { command }
    };

    vec![
        line(remove_args(shape, &server.name)),
        line(add_args(shape, &server.name, &mcp_definition(server, shape))),
    ]
}
