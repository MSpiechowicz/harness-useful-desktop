//! The Claude Code chat engine: one `claude` process per chat in stream-json mode, driven over
//! stdin/stdout ([plan](../../../../docs/plans/claude-code.md)). Not available yet:
//! [`ChatProtocol::ClaudeStream`](crate::harness::ChatProtocol::ClaudeStream) fails to start until
//! the engine is complete.
