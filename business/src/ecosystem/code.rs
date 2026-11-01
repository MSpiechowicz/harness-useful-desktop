//! The code graph of Harness Code: the switch for it, and what the hooks need to know about it.
//!
//! The server indexes a repository's code the first time an agent asks (`code_map`, `code_symbol`,
//! `code_impact`, `code_path`) and keeps the index fresh on every call. The app has two parts in it:
//!
//! - the switch: `status|enable|disable --json` prints `{"enabled", "source"}` ([`get_code_index`],
//!   [`set_code_index`]; the server keeps the choice in `code.toml` of its data folder, so its own
//!   tools agree with the app). Each asks for the server through the `code-index` feature
//!   ([`CodeFeatures::feature_server`]);
//! - the builds: [`code_builds`](super::code_builds) turns the status files the server keeps while
//!   it builds into events for a toast, and [`code_prepare`](super::code_prepare) starts a build.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::catalog::CODE_SIDECAR;
use super::code_binary::resolve_code;
use super::code_cli::{json_call, run_json};
use super::code_features::CodeFeatures;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::CodeIndexState;
use crate::process::ProcessRunner;
use crate::security::SafeReader;

/// `status --json`: whether the code index is on, and where the switch comes from. `protocol` when
/// the answer is not what the server prints.
pub async fn get_code_index(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Result<CodeIndexState> {
    let data = run_json(env, runner, server, json_call("status", &["status"])).await?;

    read_state(&data, "status")
}

/// What the hooks need to know about the code graph of the installed Harness Code.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CodeGraphSupport {
    /// The server lists the `code-index` feature (so it has the `code_*` tools).
    pub listed: bool,
    /// The Code index switch is on. Asked only when `listed`, and false when the server cannot say.
    pub on: bool,
}

/// Looks the code graph of the installed server up: the `code-index` feature, then its switch.
pub async fn code_graph_support(env: &Env, runner: &dyn ProcessRunner, features: &CodeFeatures) -> CodeGraphSupport {
    let Some(server) = resolve_code(env) else {
        return CodeGraphSupport::default();
    };

    if !features.support(env, runner).await.index() {
        return CodeGraphSupport::default();
    }

    let on = get_code_index(env, runner, &server.path)
        .await
        .is_ok_and(|state| state.enabled);

    CodeGraphSupport { listed: true, on }
}

/// Harness Code's data folder, as the server finds it: `HARNESS_CODE_DIR` when set to an absolute
/// path (a leading `~` expanded; a relative one is ignored), else `$XDG_DATA_HOME` (absolute only,
/// or `~/.local/share`) on Linux, `~/Library/Application Support` on macOS and `%LOCALAPPDATA%`
/// (absolute only) on Windows, followed by `harness-code`. `None` on Windows while `LOCALAPPDATA`
/// is unset.
pub fn code_data_dir(env: &Env) -> Option<PathBuf> {
    CODE_SIDECAR.data_dir(env)
}

/// The settings file of the switch in the server's data folder.
const SWITCH_FILE: &str = "code.toml";

/// The suffix of the variable that overrides the file in the server's environment (`0` or `1`).
const SWITCH_VAR: &str = "INDEX";

/// The most bytes read from the settings file (the server reads 4 KiB).
const SWITCH_FILE_BYTES: u64 = 4 * 1024;

/// Whether the Code index switch is on right now, read from where the server keeps it, without
/// starting a process: `HARNESS_CODE_INDEX` (`0` or `1`) of this app's environment, which the
/// agents and so their servers inherit, wins; else `enabled = bool` in `code.toml` of the server's
/// data folder. A missing file, or one without the key, is on. A file that cannot be read, is not a
/// regular file, is not valid TOML or has a wrong type is off, as the server treats it. On when the
/// data folder is unknown (the answer of `status` stands then).
pub fn code_switch_is_on(env: &Env) -> bool {
    match env.var(&CODE_SIDECAR.var(SWITCH_VAR)).map(str::trim) {
        Some("0") => return false,
        Some("1") => return true,
        _ => {}
    }

    let Some(folder) = code_data_dir(env) else {
        return true;
    };

    let path = folder.join(SWITCH_FILE);

    // A link, a folder or any other non-regular file is not a switch file the server would read.
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => return false,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return true,
        Err(_) => return false,
    }

    let Ok(text) = SafeReader::read_to_string(&path, SWITCH_FILE_BYTES) else {
        return false;
    };

    let Ok(table) = toml::from_str::<toml::Table>(&text) else {
        return false;
    };

    match table.get("enabled") {
        None => true,
        Some(toml::Value::Boolean(enabled)) => *enabled,
        Some(_) => false,
    }
}

/// `enable --json` or `disable --json`: switches the index and answers the state the server
/// reports afterwards. `process` with the server's line when it cannot write the choice.
pub async fn set_code_index(env: &Env, runner: &dyn ProcessRunner, server: &Path, on: bool) -> Result<CodeIndexState> {
    let command = if on { "enable" } else { "disable" };

    let data = run_json(env, runner, server, json_call(command, &[command])).await?;
    let state = read_state(&data, command)?;

    tracing::info!(enabled = state.enabled, source = %state.source, "code index switched");
    Ok(state)
}

fn read_state(data: &Value, name: &str) -> Result<CodeIndexState> {
    let Some(enabled) = data.get("enabled").and_then(Value::as_bool) else {
        return Err(BusinessError::Protocol(format!(
            "{} {name} answered without `enabled`",
            CODE_SIDECAR.binary
        )));
    };

    let source = data
        .get("source")
        .and_then(Value::as_str)
        .filter(|source| matches!(*source, "default" | "file" | "env"))
        .unwrap_or("default");

    Ok(CodeIndexState {
        enabled,
        source: source.to_owned(),
    })
}
