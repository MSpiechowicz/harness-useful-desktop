//! "Share setup": export the marketplaces, plugins, packages and MCP servers of each harness as a
//! `hud1:` string or a `.json` file, and import them on another machine after a review.
//!
//! - [`schema`] — the share file format v1, its limits, and lenient per-item parsing;
//! - [`codec`] — `hud1:` strings and JSON files to and from bundles (size caps, zip-bomb guard);
//! - [`validate`] — field validators shared by export (what to leave out) and import (`invalid`),
//!   with the source and MCP checks of `validate_source` and `validate_mcp`;
//! - [`secrets`] — whether a value holds anything the redactor would hide;
//! - [`collect`] — what this machine can export, read from the harnesses' files;
//! - [`plan`] — the import review of a received bundle on this machine (reads files, and runs
//!   `opencode debug config` for the MCP server names OpenCode merges);
//! - [`apply`] — applying the reviewed items the user selected (runs the harness CLIs, merges MCP
//!   files).
//!
//! Apart from [`collect`], [`plan`] and [`apply`], these modules are pure: no file IO, no
//! processes. Only references are shared, never secret values (env/header values, auth, tokens);
//! bundles are not signed, so every received item is validated here and reviewed by the user
//! before anything runs.

pub mod apply;
pub mod codec;
pub mod collect;
mod collect_items;
mod collect_mcp;
mod collect_opencode;
pub mod plan;
mod plan_presence;
mod plan_rows;
pub mod schema;
pub mod secrets;
pub mod validate;
mod validate_mcp;
mod validate_source;

pub use apply::{ApplyContext, apply_plan};
pub use collect::{
    ExportCollection, build_bundle, collect_export, export_code, export_preview, item_id, package_identity,
};
pub use plan::{BuiltPlan, OpenCodeMerged, build_plan};
pub use plan_rows::StoredPlan;
pub(crate) use validate_source::is_npm_registry_spec;
