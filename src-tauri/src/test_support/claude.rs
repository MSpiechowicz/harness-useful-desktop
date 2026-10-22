//! Claude Code in the test home: the folder trust Claude Code records in its global file.

use std::path::Path;

use harness_business::env::Env;
use harness_business::harnesses::claude::global_config_file;
use harness_business::model::HarnessId;
use serde_json::json;

/// For a Claude Code setup, records in the test home that the user trusted `project` in Claude
/// Code (a chat there starts without trust once, other folders such as the routines folder stay
/// untrusted).
pub(super) fn trust_project(env: &Env, harness: HarnessId, project: &Path) {
    if harness != HarnessId::Claude {
        return;
    }

    let trusted = json!({ "projects": { project.to_string_lossy(): { "hasTrustDialogAccepted": true } } });

    std::fs::write(global_config_file(env), trusted.to_string()).expect("write the Claude global file");
}
