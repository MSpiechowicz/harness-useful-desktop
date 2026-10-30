//! Setup, "Share setup" sync, the Harness family ecosystem and harness-memory.
//!
//! One test binary: each module is a test file, the shared helpers live in `tests/support`.

#[path = "../support/claude_support/mod.rs"]
mod claude_support;
#[path = "../support/codex_support/mod.rs"]
mod codex_support;
#[path = "../support/common/mod.rs"]
mod common;
#[path = "../support/sync_support/import.rs"]
mod import_support;
#[path = "../support/mcp_support/mod.rs"]
mod mcp_support;
#[path = "../support/memory_cli_support/mod.rs"]
mod memory_cli_support;
#[path = "../support/memory_notes_copy_support/mod.rs"]
mod memory_notes_copy_support;
#[path = "../support/memory_remote_support/mod.rs"]
mod memory_remote_support;
#[path = "../support/memory_stand_in/mod.rs"]
mod memory_stand_in;
#[path = "../support/recording_cli/mod.rs"]
mod recording_cli;
#[path = "../support/setup_support/mod.rs"]
mod setup_support;
#[path = "../support/sync_support/mod.rs"]
mod sync_support;
#[path = "../support/team_support/mod.rs"]
mod team_support;

mod desktop_register;
mod ecosystem_detect;
mod ecosystem_model;
mod ecosystem_releases;
mod live_ecosystem;
mod mcp_merge;
mod memory_code;
mod memory_code_prepare;
mod memory_code_switch;
mod memory_embedder;
mod memory_embedder_supervisor;
mod memory_features;
mod memory_hooks;
mod memory_kinds;
mod memory_list_filters;
mod memory_migrate;
mod memory_notes;
mod memory_notes_copy;
mod memory_notes_hidden;
mod memory_notes_useful;
mod memory_record;
mod memory_register;
mod memory_remote;
mod memory_remote_check;
mod memory_remote_documentdb;
mod memory_remote_routing;
mod memory_remote_scripts;
mod memory_remote_scripts_routing;
mod memory_remote_team_docs;
mod memory_remote_url;
mod memory_requests;
mod memory_save;
mod memory_save_scripts;
mod memory_scripts;
mod memory_scripts_copy;
mod memory_scripts_hidden;
mod memory_scripts_start;
mod memory_server;
mod memory_team_docs_mine;
mod memory_trust;
mod memory_user_save;
mod setup_codex_skills;
mod setup_codex_skills_names;
mod setup_installer;
mod setup_mcp_register;
mod setup_mcp_register_claude;
mod setup_mcp_register_codex;
mod setup_mcp_register_opencode;
mod setup_mcp_register_opencode2;
mod setup_opencode_skills;
mod setup_status;
mod setup_status_claude;
mod skills_migration;
mod sync_codec;
mod sync_export;
mod sync_export_claude;
mod sync_export_codex;
mod sync_export_limits;
mod sync_export_opencode;
mod sync_export_opencode2;
mod sync_export_paths;
mod sync_export_reserved;
mod sync_import;
mod sync_import_apply;
mod sync_import_claude;
mod sync_import_codex;
mod sync_import_mcp_layers;
mod sync_import_opencode;
mod sync_import_opencode2;
mod sync_import_opencode_merged;
mod sync_import_replace;
mod sync_import_replace_cli;
mod sync_import_replace_whole;
mod sync_validate;
mod sync_validate_mcp;
mod sync_validate_specs;
mod updates;
