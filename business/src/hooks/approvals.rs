//! `<app_config_dir>/hook-approvals.json`: `{ "version": 1, "approvals": { "<id>": "<sha256>" } }`
//! ([`crate::signed_docs::approvals`] for hooks).
//!
//! Written only by the app window (the MCP server reads it). A hook runs only while the SHA-256 of
//! its exact file bytes equals its entry, so any byte change (an agent rewriting the same values
//! included) makes it pending again. An invalid file is renamed
//! `hook-approvals.json.corrupt-<unix-seconds>` by the next write, and every hook is pending until
//! the user approves it again.
//!
//! Beside it, `<app_config_dir>/hook-approved/<id>.json` keeps a copy of the approved bytes, so the
//! Hooks view can show what changed in a pending edit ([`approved_snapshot`]).

use std::path::Path;

use super::HookKind;
use crate::model::Hook;
use crate::signed_docs::snapshots::approved_doc;

/// The folder of approved copies in the app config folder.
pub const SNAPSHOT_FOLDER: &str = "hook-approved";

/// The approved hash of each hook.
pub type Approvals = crate::signed_docs::Approvals<HookKind>;

/// The approved version of hook `id`: the kept copy in `config_dir`, when it still hashes to
/// `approved_sha256` and parses.
pub fn approved_snapshot(config_dir: &Path, id: &str, approved_sha256: &str) -> Option<Hook> {
    approved_doc::<HookKind>(config_dir, id, approved_sha256)
}
