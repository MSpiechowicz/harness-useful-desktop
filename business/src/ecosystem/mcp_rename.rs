//! The dashboard's MCP server under its old name, `harness`, in a harness's user MCP file: part of
//! the launch migration ([`super::skills_migration`]), which moves it to [`DASHBOARD_MCP_SERVER`]
//! where the user had added it under the shorter name.
//!
//! - Found: a server named [`LEGACY_DASHBOARD_MCP_SERVER`] in the target file (never an
//!   `also_read` one, which the app does not write) that runs `harness-dashboard mcp` (see
//!   [`runs_mcp`]), and either no server named [`DASHBOARD_MCP_SERVER`] in any of the files or one
//!   with the identical definition (a rename that stopped after the add, finished by the removal
//!   alone). A different `harness-dashboard` server is the user's own and keeps the old one where
//!   it is, so does a server some file lists in `disabledServers` or `enabledServers`.
//! - File-backed shapes ([`Shape::app_writes`]): [`rename_dashboard_mcp_file`] writes the file once
//!   ([`change_file`]'s rules), so the server is never absent in between.
//! - CLI-backed shapes: [`plan_rename`] says what to add and remove, and the caller runs the
//!   harness's CLI add first and remove second, so a failure between them leaves both and never
//!   neither.
//!
//! The entry is copied as it is; only the name changes. A server's definition can hold secrets
//! (`env`), so it is never logged.

use std::path::Path;

use serde_json::Value;

use super::catalog::{DASHBOARD_BINARY, DASHBOARD_MCP_SERVER, LEGACY_DASHBOARD_MCP_SERVER};
use super::mcp_merge::{Blocked, Change, Shape, Target, change_file, load, remove_server, servers};
use super::mcp_register::{add_server, runs_mcp};
use super::setup::McpFile;
use crate::model::McpRefusal;
use crate::pi_family::string_list;

/// What renaming the old server takes.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardRename {
    /// The entry as the old server holds it.
    pub entry: Value,
    /// False when a server of the new name with the identical entry exists, so only the old one is
    /// removed.
    pub add: bool,
}

/// The rename `file`, read as `target`, needs; `None` when there is nothing to rename (see the
/// module docs).
pub(crate) fn plan_rename(file: &McpFile, target: &Target) -> Option<DashboardRename> {
    let document = target.document();
    let known = servers(&document, file.shape)?;
    let old = known.get(LEGACY_DASHBOARD_MCP_SERVER)?;

    if !runs_mcp(old, DASHBOARD_BINARY) {
        return None;
    }

    let listed = ["disabledServers", "enabledServers"]
        .iter()
        .flat_map(|key| string_list(document.get(*key)))
        .any(|name| name == LEGACY_DASHBOARD_MCP_SERVER);

    if listed {
        return None;
    }

    let add = match known.get(DASHBOARD_MCP_SERVER) {
        Some(new) if new == old => false,
        Some(_) => return None,
        None => true,
    };

    if add && file.also_read.iter().any(|other| holds_new_name(other, file.shape)) {
        return None;
    }

    Some(DashboardRename {
        entry: entry_for(file.shape, old),
        add,
    })
}

/// Renames the old server in `file`, written by the app. `Ok(true)` when it was renamed,
/// `Ok(false)` when there was nothing to do.
pub fn rename_dashboard_mcp_file(file: &McpFile) -> Result<bool, Blocked> {
    change_file(&file.path, file.shape, |target| {
        let Some(rename) = plan_rename(file, target) else {
            return Change::Keep(false);
        };

        let mut document = target.document();
        remove_server(&mut document, file.shape, LEGACY_DASHBOARD_MCP_SERVER, |old| {
            runs_mcp(old, DASHBOARD_BINARY)
        });

        if rename.add {
            add_server(&mut document, file.shape, DASHBOARD_MCP_SERVER, rename.entry);
        }

        Change::Write(document, true)
    })
}

/// Reads `file` for [`plan_rename`]; the refusal when it cannot be read.
pub(crate) fn read_rename_plan(file: &McpFile) -> Result<Option<DashboardRename>, McpRefusal> {
    let target = load(&file.path, file.shape)?;

    Ok(plan_rename(file, &target))
}

/// Whether the `also_read` file at `path` holds a server named [`DASHBOARD_MCP_SERVER`].
fn holds_new_name(path: &Path, shape: Shape) -> bool {
    match load(path, shape) {
        Ok(target) => servers(&target.document(), shape).is_some_and(|known| known.contains_key(DASHBOARD_MCP_SERVER)),
        Err(_) => false,
    }
}

/// The entry to write under the new name. OpenCode 2 has no `enabled` (a legacy `mcp.harness`
/// entry may carry it): `enabled: true` is dropped and `enabled: false` becomes `disabled: true`.
fn entry_for(shape: Shape, old: &Value) -> Value {
    let mut entry = old.clone();

    if shape != Shape::OpenCodeV2 {
        return entry;
    }

    let Some(fields) = entry.as_object_mut() else {
        return entry;
    };

    if let Some(enabled) = fields.remove("enabled")
        && enabled == Value::Bool(false)
    {
        fields.insert("disabled".to_owned(), Value::Bool(true));
    }

    entry
}
