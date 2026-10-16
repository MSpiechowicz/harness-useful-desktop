//! `harness-memory`: a project memory MCP server for any coding agent harness.
//!
//! Agents remember outcomes, problems, fixes, locations, optimisations and decisions as short
//! notes, search them with full-text ranking, and relate entities in a small graph. Notes belong
//! to one repository (identified by its `origin` remote, see [`project`]) or to the global scope.
//!
//! - [`store`]: the [`store::MemoryStore`] contract and its SQLite backend (WAL, FTS5).
//! - [`mcp`]: the stdio JSON-RPC server and its tools.
//! - [`limits`]: per-call limits (the amount of memory itself is never capped).
//!
//! The database lives in the app's per-machine data folder ([`database_path`]), never in a
//! repository; everything stored passes through `redact_text` first.

pub mod limits;
pub mod mcp;
pub mod project;
pub mod store;

use std::path::PathBuf;

use harness_business::app_dirs::memory_dir;
use harness_business::env::Env;

pub use mcp::{Server, serve};

/// The database file: `<memory dir>/memory.sqlite3`; `None` when the data folder is unknown.
pub fn database_path(env: &Env) -> Option<PathBuf> {
    memory_dir(env).map(|dir| dir.join("memory.sqlite3"))
}
