//! Business layer of Harness Desktop: harness adapters, agent sessions, git, security.
//! No Tauri dependency; the desktop shell is a thin layer of IPC commands over this crate.
//!
//! # Layering
//!
//! 1. [`model`] — serde + ts-rs DTOs that form the IPC contract (generated TypeScript bindings).
//! 2. Foundations used by every domain module:
//!    - [`error`] / [`env`] — the crate error type and the injected environment ([`env::Env`]);
//!    - [`security`] — redaction, the never-read deny-list with [`security::SafeReader`], and path
//!      containment;
//!    - [`process`] — shell-free process execution behind injectable traits.
//! 3. Domain modules built on the foundations: [`prefs`]; the harness abstraction ([`harness`])
//!    with its adapters ([`harnesses`]) and the parsing and RPC protocol they share
//!    ([`pi_family`], including saved sessions on disk); running agent chat sessions over that
//!    protocol ([`agent`]); the project repository through the git CLI ([`git`]); diff review
//!    prompts ([`review`]); the companion tools and the dashboard's data ([`ecosystem`]); the
//!    in-app updater's key check and progress reporting ([`update`]); sharing a setup between
//!    machines ([`sync`]).
//!
//! Rules every module follows: no shell is ever spawned, every file read goes through
//! [`security::SafeReader`], every path is derived from an injected [`env::Env`], and nothing
//! secret is returned to the webview unredacted.

pub mod agent;
pub mod app_dirs;
pub mod attachments;
pub mod dropped;
pub mod ecosystem;
pub mod env;
pub mod error;
pub mod fs_atomic;
pub mod git;
pub mod harness;
pub mod harnesses;
pub mod hooks;
pub mod hooks_mcp;
pub mod model;
pub mod pi_family;
pub mod prefs;
pub mod process;
pub mod project;
pub mod review;
pub mod routines;
pub mod security;
pub mod signed_docs;
pub mod sync;
pub mod update;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use error::{BusinessError, Result};
