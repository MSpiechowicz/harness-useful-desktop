//! Harness Memory in the shell's unit tests: a data folder and the run requests its server writes.

use std::path::{Path, PathBuf};

use harness_business::env::Env;

use super::{TempDir, set_mode};

/// The id of the run request [`write_script_request`] writes in tests.
pub const REQUEST_ID: &str = "0123456789abcdef0123456789abcdef";

/// A temp folder with a home and a Harness Memory data folder (`HARNESS_MEMORY_DIR`, not created).
pub fn memory_data_setup(label: &str) -> (TempDir, Env, PathBuf) {
    let dir = TempDir::new(label);
    let home = dir.path().join("home");
    let data = dir.path().join("memory-data");
    let env = Env::for_tests(&home).with_var("HARNESS_MEMORY_DIR", data.to_string_lossy());

    (dir, env, data)
}

/// Writes run request `id` for script `deploy` into `<data>/requests` (created `0700`), as the
/// server does.
pub fn write_script_request(data: &Path, id: &str) {
    let requests = data.join(harness_business::ecosystem::memory_requests::REQUESTS_DIR);
    std::fs::create_dir_all(&requests).expect("create requests");
    set_mode(&requests, 0o700);

    let request = serde_json::json!({
        "version": 1, "requestId": id, "createdAt": "2026-10-17T10:00:00Z", "client": "codex",
        "script": {
            "id": 3, "name": "deploy", "description": "", "language": "sh", "scope": "global",
            "repo": null, "sha256": "00", "timeoutSecs": 60, "place": "team",
            "author": { "source": "codex", "root": null }, "createdAt": "x", "updatedAt": "x",
            "runCount": 0, "lastRunAt": null, "lastExitCode": null, "trusted": false, "approval": "needed"
        },
        "body": "echo hi\n", "language": "sh", "args": [], "previous": null
    });
    std::fs::write(requests.join(format!("{id}.json")), request.to_string()).expect("write request");
}
