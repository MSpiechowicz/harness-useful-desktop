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
//! 3. Domain modules built on the foundations (currently [`prefs`]; harness adapters, sessions,
//!    git and ecosystem follow in later milestones).
//!
//! Rules every module follows: no shell is ever spawned, every file read goes through
//! [`security::SafeReader`], every path is derived from an injected [`env::Env`], and nothing
//! secret is returned to the webview unredacted.

pub mod env;
pub mod error;
pub mod model;
pub mod prefs;
pub mod process;
pub mod security;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use error::{BusinessError, Result};
