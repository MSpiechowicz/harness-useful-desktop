//! Harness config, extension and session views, the stream mappers, canaries and the deny-list.
//!
//! One test binary: each module is a test file, the shared helpers live in `tests/support`.

#[path = "../support/agent_support/mod.rs"]
mod agent_support;
#[path = "../support/claude_frames/mod.rs"]
mod claude_frames;
#[path = "../support/claude_mapper_support/mod.rs"]
mod claude_mapper_support;
#[path = "../support/claude_recorded/mod.rs"]
mod claude_recorded;
#[path = "../support/claude_session_support/mod.rs"]
mod claude_session_support;
#[path = "../support/claude_support/mod.rs"]
mod claude_support;
#[path = "../support/codex_support/mod.rs"]
mod codex_support;
#[path = "../support/common/mod.rs"]
mod common;
#[cfg(unix)]
#[path = "../support/fake_cli_support/mod.rs"]
mod fake_cli_support;
#[path = "../support/git_support/mod.rs"]
mod git_support;
#[path = "../support/mock_support/mod.rs"]
mod mock_support;
#[path = "../support/opencode2_mapper_support/mod.rs"]
mod opencode2_mapper_support;
#[path = "../support/opencode2_recorded/mod.rs"]
mod opencode2_recorded;
#[path = "../support/opencode_mapper_support/mod.rs"]
mod opencode_mapper_support;
#[path = "../support/opencode_session_support/mod.rs"]
mod opencode_session_support;
#[path = "../support/recording_cli/mod.rs"]
mod recording_cli;
#[path = "../support/samples/mod.rs"]
mod samples;
#[path = "../support/session_support/mod.rs"]
mod session_support;
#[path = "../support/setup_support/mod.rs"]
mod setup_support;

mod adapter_canary;
mod canary;
mod claude_cli_actions;
mod claude_extensions;
mod claude_global_file;
mod claude_mapper;
mod claude_mapper_crafted;
mod claude_mapper_robustness;
mod claude_protected_asks;
mod claude_skills_config;
mod claude_stats;
mod claude_tools;
mod claude_views;
mod cli_actions;
mod codex_cli_actions;
mod codex_trust_views;
mod codex_views;
mod dashboard_query;
mod deny_claude;
mod deny_codex;
mod mock_fixtures;
mod omp_views;
mod opencode2_asks_mapper;
mod opencode2_mapper;
mod opencode2_merged_names;
mod opencode2_plugin_actions;
mod opencode2_plugin_remove;
mod opencode2_quiet_mapper;
mod opencode2_tools_mapper;
mod opencode2_turns_mapper;
mod opencode2_views;
mod opencode_asks_mapper;
mod opencode_generation;
mod opencode_mapper;
mod opencode_plugin_actions;
mod opencode_plugin_remove;
mod opencode_quiet_mapper;
mod opencode_skills_config;
mod opencode_turns_mapper;
mod opencode_views;
mod pi_packages;
mod pi_views;
mod session_paths;
mod sessions_claude;
mod sessions_claude_transcript;
mod sessions_codex;
mod sessions_codex_paging;
mod sessions_codex_parents;
mod sessions_copies;
mod sessions_opencode;
mod sessions_opencode2;
mod sessions_opencode2_transcript;
mod sessions_opencode_concurrency;
mod sessions_opencode_transcript;
mod sessions_store;
mod sessions_totals;
