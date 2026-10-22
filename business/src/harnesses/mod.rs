//! Concrete harness adapters, one module per CLI.
//!
//! To add a harness, create `harnesses/<name>/` with a type implementing
//! [`crate::harness::Harness`], export it here and register it in
//! [`crate::harness::HarnessRegistry::with_defaults`]. The Claude-compatible plugin registry and
//! marketplace catalog formats OMP and Claude Code share live in [`plugin_registry`], and what
//! Harness Skills keeps for Claude Code and OpenCode in `skills_config`.

pub mod claude;
pub mod omp;
pub mod opencode;
pub mod pi;
pub mod plugin_registry;
mod skills_config;

pub use claude::Claude;
pub use omp::Omp;
pub use opencode::OpenCode;
pub use pi::Pi;
