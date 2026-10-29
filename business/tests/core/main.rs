//! Projects, prefs, processes, git, hooks, routines and the other harness-independent tests.
//!
//! One test binary: each module is a test file, the shared helpers live in `tests/support`.
#![expect(
    clippy::module_inception,
    reason = "the hooks test files keep their tests in a module of their own name"
)]

#[path = "../support/common/mod.rs"]
mod common;
#[path = "../support/git_support/mod.rs"]
mod git_support;
#[path = "../support/hook_store_support/mod.rs"]
mod hook_store_support;
#[path = "../support/hook_support/mod.rs"]
mod hook_support;
#[path = "../support/memory_cli_support/mod.rs"]
mod memory_cli_support;
#[path = "../support/process_support/mod.rs"]
mod process_support;
#[path = "../support/routine_support/mod.rs"]
mod routine_support;
#[path = "../support/session_support/mod.rs"]
mod session_support;
#[path = "../support/team_support/mod.rs"]
mod team_support;

mod blocking_runner;
mod catalog;
mod detection;
mod detection_codex;
mod env_capture;
mod git_branch_search;
mod git_claude_private;
mod git_diff_parse;
mod git_env;
mod git_hostile;
mod git_repo;
mod git_sync;
mod git_sync_actions;
mod git_sync_background;
mod git_sync_failures;
mod git_sync_remotes;
mod git_sync_stash;
mod harness_paths;
mod hooks_agent;
mod hooks_command;
mod hooks_compose;
mod hooks_engine;
mod hooks_family;
mod hooks_family_code_graph;
mod hooks_follow_up;
mod hooks_format;
mod hooks_mcp;
mod hooks_mcp_capabilities;
mod hooks_mcp_scope;
mod hooks_memory_gate;
mod hooks_memory_notice;
mod hooks_runs;
mod hooks_store;
mod hooks_store_folder;
mod hooks_team;
mod hooks_team_actions;
mod hooks_team_sync;
mod live_omp;
mod managed_prefs;
mod model;
mod prefs;
mod process;
mod process_orphans;
mod project_scopes;
mod projects;
mod real_home_smoke;
mod recall_block;
mod redact;
mod resolve;
mod review_compose;
mod routines_cron;
mod routines_folder_owner;
mod routines_format;
mod routines_mcp;
mod routines_mcp_modes;
mod routines_mcp_scope;
mod routines_plan;
mod routines_runs;
mod routines_session_tags;
mod routines_store;
mod routines_team;
mod routines_team_confirm;
mod routines_when;
mod routines_workspace;
mod safe_reader;
mod subagent_contract;
mod team_listing_all;
