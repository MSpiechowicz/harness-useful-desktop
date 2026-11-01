//! Test helpers for Harness Code's data folder.

use std::path::PathBuf;

use harness_business::env::Env;

use super::TempDir;

/// A temporary home and a data folder for Harness Code beside it (`HARNESS_CODE_DIR`), which the
/// test creates when it needs it.
pub fn code_data_setup(label: &str) -> (TempDir, Env, PathBuf) {
    let dir = TempDir::new(label);
    let home = dir.path().join("home");
    let data = dir.path().join("code-data");
    let env = Env::for_tests(&home).with_var("HARNESS_CODE_DIR", data.to_string_lossy());

    (dir, env, data)
}
