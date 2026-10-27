//! The apply half of "Share setup": what `sync_import_apply` did, item by item, and the progress it
//! streams. See `sync.rs` for the shared types and the item ids.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{HarnessId, McpRefusal, SyncItemKind, SyncNote};

/// What `sync_import_apply` did. There is no rollback: items that succeeded stay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncImportResult {
    /// True when every selected item ended `done`, `replaced` or `alreadyPresent`.
    pub ok: bool,
    /// One entry per selected item, in the order they were applied.
    pub items: Vec<SyncItemOutcome>,
}

/// How one selected item ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncItemOutcome {
    /// The item id from the plan.
    pub id: String,
    /// Its harness.
    pub harness: HarnessId,
    /// What it is.
    pub kind: SyncItemKind,
    /// Its label from the plan.
    pub label: String,
    /// The outcome.
    pub outcome: SyncOutcome,
    /// The CLI's short, redacted outcome line, when a command ran.
    pub summary: Option<String>,
    /// The command line that ran (program name first), when one ran.
    pub argv: Vec<String>,
    /// Remarks on the outcome (`configUnreadable` on a `conflict`).
    pub notes: Vec<SyncNote>,
}

/// The outcome of one item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum SyncOutcome {
    /// Installed or added.
    Done,
    /// An existing MCP server of that name was replaced (for an HTTP or SSE server at the same endpoint its
    /// other settings were kept, else the whole entry was replaced).
    Replaced,
    /// Found already present when re-checked right before its step; nothing ran.
    AlreadyPresent,
    /// Something with the same name that the preview did not allow replacing is here (it appeared
    /// since the preview, or is in a layer this app does not write), or (note `configUnreadable`)
    /// a file that could hold one cannot be read; left alone.
    Conflict,
    /// Not attempted.
    Skipped {
        /// Why.
        reason: SyncSkipReason,
    },
    /// The step ran and failed (see `summary`).
    Failed,
}

/// Why a selected item was not attempted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum SyncSkipReason {
    /// The marketplace it depends on failed to register.
    DependencyFailed,
    /// The harness is no longer installed.
    HarnessNotInstalled,
    /// The harness's MCP file cannot be changed safely; nothing was written.
    McpFileRefused {
        /// Why.
        refusal: McpRefusal,
    },
    /// The harness's config file has comments a rewrite would lose (OpenCode's JSONC), so nothing
    /// was written: the user pastes `snippet` into its servers object (`mcp`).
    ManualEdit {
        /// The file's home-relative display path (e.g. `"~/.config/opencode/opencode.jsonc"`).
        file: String,
        /// The server entry to add, as text ready to paste (`"<name>": { … }`).
        snippet: String,
    },
    /// Like `manualEdit`, but for replacing an existing HTTP or SSE entry of the same endpoint: the user
    /// replaces these fields in the server's entry of `file` and keeps the others. (For another
    /// endpoint the whole entry is to be replaced, which is a `manualEdit` with the full entry.)
    ManualReplace {
        /// The file's home-relative display path.
        file: String,
        /// The managed fields to set (`"type": …, "command": …`), as text ready to paste; no
        /// `enabled`/`disabled` and nothing read from the existing entry.
        snippet: String,
    },
}

/// Progress of `sync_import_apply`, streamed while it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum SyncProgress {
    /// An item's step is starting.
    Started {
        /// The item id.
        id: String,
        /// Its position among the selected items, from 0.
        index: u32,
        /// How many items are selected.
        total: u32,
    },
    /// An item finished.
    Finished {
        /// How it ended.
        outcome: SyncItemOutcome,
    },
}
