//! The chat engines (OMP/PI over RPC, OpenCode, Claude Code, Codex, MCP) against `fake_agent` and the
//! recorded servers, with their launch, wire and stream code.
//!
//! One test binary: each module is a test file, the shared helpers live in `tests/support`.

#[path = "../support/agent_support/mod.rs"]
mod agent_support;
#[path = "../support/claude_engine_support/mod.rs"]
mod claude_engine_support;
#[path = "../support/claude_recorded/mod.rs"]
mod claude_recorded;
#[path = "../support/codex_engine_support/mod.rs"]
mod codex_engine_support;
#[path = "../support/codex_recorded/mod.rs"]
mod codex_recorded;
#[path = "../support/common/mod.rs"]
mod common;
#[path = "../support/fake_opencode2_support/mod.rs"]
mod fake_opencode2_support;
#[path = "../support/opencode2_engine_helpers/mod.rs"]
mod opencode2_engine_helpers;
#[path = "../support/opencode2_engine_support/mod.rs"]
mod opencode2_engine_support;
#[path = "../support/opencode2_recorded/mod.rs"]
mod opencode2_recorded;
#[path = "../support/opencode_engine_support/mod.rs"]
mod opencode_engine_support;
#[path = "../support/opencode_http_support/mod.rs"]
mod opencode_http_support;
#[path = "../support/opencode_mapper_support/mod.rs"]
mod opencode_mapper_support;

mod agent_dialogs;
mod agent_failures;
mod agent_launch;
mod agent_manager;
mod agent_memory_session;
mod agent_memory_session_codex;
mod agent_session;
mod agent_session_ids;
mod claude_allow_list;
mod claude_chat_start;
mod claude_engine;
mod claude_engine_asks;
mod claude_engine_bash;
mod claude_engine_modes;
mod claude_engine_ops;
mod claude_engine_queue;
mod claude_engine_robustness;
mod claude_engine_roles;
mod claude_engine_session_owner;
mod claude_engine_settings;
mod claude_engine_subagents;
mod claude_engine_turns;
mod claude_launch;
mod claude_wire;
mod claude_wire_encode;
mod codex_engine;
mod codex_engine_asks;
mod codex_engine_history;
mod codex_engine_memory;
mod codex_engine_mode_warnings;
mod codex_engine_models;
mod codex_engine_modes;
mod codex_engine_ops;
mod codex_engine_ops_modes;
mod codex_engine_queue;
mod codex_mapper;
mod codex_mapper_asks;
mod codex_mapper_crafted;
mod codex_naming;
mod codex_trust;
mod codex_wire;
mod fake_codex;
mod fake_opencode;
mod fake_opencode2;
mod mcp_client;
mod mcp_client_cancel;
mod omp_subagent_steps;
mod opencode2_engine;
mod opencode2_engine_asks;
mod opencode2_engine_catch_up;
mod opencode2_engine_commands;
mod opencode2_engine_helper_sessions;
mod opencode2_engine_modes;
mod opencode2_engine_ops;
mod opencode2_engine_queue;
mod opencode2_engine_reports;
mod opencode2_engine_runs;
mod opencode2_engine_stream;
mod opencode2_engine_subagents;
mod opencode2_engine_tools;
mod opencode2_engine_turns;
mod opencode2_events;
mod opencode2_modes;
mod opencode2_routes;
mod opencode2_wire;
mod opencode_engine;
mod opencode_engine_asks;
mod opencode_engine_commands;
mod opencode_engine_modes;
mod opencode_engine_ops;
mod opencode_engine_probe;
mod opencode_engine_queue;
mod opencode_engine_runs;
mod opencode_engine_subagents;
mod opencode_engine_turns;
mod opencode_engine_watchdog;
mod opencode_http;
mod opencode_launch;
mod opencode_mode_config;
mod opencode_modes;
mod opencode_reports;
mod opencode_routes;
mod opencode_sse;
mod opencode_wire;
mod rpc_codec;
mod rpc_commands;
mod rpc_dialects;
mod rpc_history;
mod rpc_normalize;
