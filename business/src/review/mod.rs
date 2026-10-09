//! Diff review comments composed into an agent prompt (`review_compose`).
//!
//! The webview sends the composed prompt itself with `agent_send` (as a prompt, or a follow-up
//! while the agent is busy); nothing here talks to an agent.

mod compose;

pub use compose::{INTRO, compose_review, language};
