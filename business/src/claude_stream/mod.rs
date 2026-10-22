//! Talking to Claude Code's stream-json mode with its control protocol, the base of the Claude chat
//! engine ([plan](../../../docs/plans/claude-code.md)): pure code for the wire frames and control
//! requests, the mapper from frames to chat events, tool ids and touched paths, approval-mode
//! mapping and session statistics. Empty until the chat engine steps of the plan.

pub mod tools;
