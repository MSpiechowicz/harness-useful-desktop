//! The Claude-compatible plugin formats OMP and Claude Code share.
//!
//! - [`installed`]: `installed_plugins.json`, version 2: `{ version: 2, plugins: { "name@marketplace":
//!   [{ scope, installPath, version, installedAt, lastUpdated, gitCommitSha, projectPath? }] } }`.
//! - [`catalog`]: marketplace catalogs (`{ plugins: [{ name, version?, description?, source }] }`),
//!   the update marks they give installed plugins, and how a marketplace source is shown.
//!
//! What a registry entry means (enabled state, which scopes apply, where plugin directories may
//! live) differs per harness and stays in the adapters. Every file is read through
//! [`crate::security::SafeReader`].

pub mod catalog;
pub mod installed;

pub use catalog::{
    CatalogError, GITHUB_REPO, contained_catalog, display_source, is_newer, is_word_like, mark_updates, read_catalog,
};
pub use installed::{Registered, install_path, read_registry, split_id, verified_dir};
