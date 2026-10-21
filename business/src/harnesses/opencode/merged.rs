//! The MCP server names OpenCode merges from all its config layers, read from
//! `opencode debug config`, which prints the resolved config as JSON. OpenCode masks some secret
//! values in it, not all: the output is never logged, stored or returned.
//!
//! The command is not read-only: OpenCode may fetch its organization config over the network,
//! start a dependency install (`bun add @opencode-ai/plugin`) in its config directories, create
//! `opencode.jsonc`, add `$schema`, or migrate a legacy TOML config. It starts OpenCode fully, so
//! the configured plugins load and run (including any the same import just installed), and a
//! plugin's config hook can change what it reports. "Share setup" runs it only while applying an
//! import the user confirmed, never while reviewing one.
//!
//! OpenCode also merges layers the app cannot read: a remote organization config (`opencode auth
//! login`), managed settings, `OPENCODE_CONFIG_CONTENT` and the project layers of the directory it
//! runs in. A server imported under the name of one of those would be deep-merged with it, keeping
//! its headers or env. "Share setup" checks the names here before writing an OpenCode server.
//!
//! The command runs in the home directory (as the app's other OpenCode commands), with OpenCode's
//! spawn environment (no self-update), no colors, the launcher search directories, a
//! [`PROBE_TIMEOUT`] and a [`PROBE_MAX_BYTES`] cap. Its stdout goes through a file: OpenCode keeps
//! only the first 64 KiB of it in a pipe. The output is config: only the names under `mcp` are
//! kept, and errors never repeat any of it.
//!
//! OpenCode 2 (2.1.2) prints no resolved config, and its `debug config` starts its background
//! service, which the app never does. It runs `opencode api --standalone GET /api/config` instead
//! ([`PROBE_ARGS_V2`], `cli/src/commands/handlers/api.ts`): `--standalone` starts a private `opencode
//! serve --stdio --port 0` with a random password, child of the command, which ends with it, and
//! never contacts the background service. The answer lists the config sources the server merged
//! for its directory (the home directory here), each `{type: "directory", path}` or `{type:
//! "document", path?, info}`, `info` being that layer's config normalized to OpenCode 2's keys. The
//! names are those of `mcp.servers` of every document, plus any legacy `mcp.<name>` server (see
//! [`super::config`]); a server switched off with `disabled` still counts, since a new entry of its
//! name would merge with it. The same environment, limits and care as above apply, and the command
//! is just as little read-only: the private server loads OpenCode fully, plugins included. Its
//! private server also skips the project config, the model catalog download and the file watcher
//! (as every OpenCode 2 command of the app), so the home directory is not walked as a project:
//! `~/.opencode` and `~/opencode.json[c]` are still checked, as files the app reads itself, but a
//! server only in a config file of a parent of the home directory is not seen.

use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::Value;

use super::config::mcp_servers;
use super::{ONE_SHOT_ENV_V2, SPAWN_ENV};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harness::OpenCodeGeneration;
use crate::process::{ExecSpec, ProcessRunner, ResolvedBinary, launcher_search_dirs};

/// The arguments of the probe.
pub const PROBE_ARGS: [&str; 2] = ["debug", "config"];

/// The arguments of the OpenCode 2 probe.
pub const PROBE_ARGS_V2: [&str; 4] = ["api", "--standalone", "GET", "/api/config"];

/// Time limit of the probe.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// Output cap of the probe; a larger config fails it.
pub const PROBE_MAX_BYTES: usize = 4 * 1024 * 1024;

/// The names of the MCP servers in OpenCode's resolved config. An error when the command could not
/// run, failed, timed out, printed more than [`PROBE_MAX_BYTES`] or something that is not a JSON
/// object whose `mcp`, if any, is an object (OpenCode 2: not a list of config sources whose
/// servers are objects).
pub async fn merged_mcp_names(
    env: &Env,
    runner: &dyn ProcessRunner,
    binary: &ResolvedBinary,
    generation: OpenCodeGeneration,
) -> Result<BTreeSet<String>> {
    let (args, command): (&[&str], &str) = match generation {
        OpenCodeGeneration::V1 => (&PROBE_ARGS, "opencode debug config"),
        OpenCodeGeneration::V2 => (&PROBE_ARGS_V2, "opencode api --standalone"),
    };

    let one_shot_env = match generation {
        OpenCodeGeneration::V1 => &[][..],
        OpenCodeGeneration::V2 => ONE_SHOT_ENV_V2,
    };

    let spec = ExecSpec::new(&binary.path)
        .args(args.iter().copied())
        .cwd(&env.home)
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0")
        .envs(SPAWN_ENV.iter().copied())
        .envs(one_shot_env.iter().copied())
        .search_dirs(launcher_search_dirs(env, &binary.path))
        .timeout(PROBE_TIMEOUT)
        .max_output_bytes(PROBE_MAX_BYTES)
        .stdout_through_file();

    let output = runner.run(&spec).await?;

    if output.timed_out {
        return Err(BusinessError::Timeout(command.to_owned()));
    }

    if !output.success() {
        return Err(BusinessError::Process(format!("{command} failed")));
    }

    if output.truncated {
        return Err(BusinessError::InvalidInput(format!(
            "the output of {command} is too large"
        )));
    }

    match generation {
        OpenCodeGeneration::V1 => mcp_names(&output.stdout),
        OpenCodeGeneration::V2 => source_mcp_names(&output.stdout),
    }
}

/// The keys of `mcp` in the printed config.
fn mcp_names(stdout: &str) -> Result<BTreeSet<String>> {
    let unreadable = || BusinessError::Protocol("the output of opencode debug config is not a config".to_owned());

    let Ok(Value::Object(config)) = serde_json::from_str::<Value>(stdout.trim()) else {
        return Err(unreadable());
    };

    match config.get("mcp") {
        None => Ok(BTreeSet::new()),
        Some(Value::Object(servers)) => Ok(servers.keys().cloned().collect()),
        Some(_) => Err(unreadable()),
    }
}

/// The server names of every document among the printed OpenCode 2 config sources (a bare list,
/// or one wrapped in `{data}`). Other source types are skipped.
fn source_mcp_names(stdout: &str) -> Result<BTreeSet<String>> {
    let unreadable =
        || BusinessError::Protocol("the output of opencode api is not a list of config sources".to_owned());

    let printed: Value = serde_json::from_str(stdout.trim()).map_err(|_| unreadable())?;
    let sources = match &printed {
        Value::Array(sources) => sources,
        Value::Object(wrapped) => wrapped.get("data").and_then(Value::as_array).ok_or_else(unreadable)?,
        _ => return Err(unreadable()),
    };

    let mut names = BTreeSet::new();

    for source in sources {
        if source.get("type").and_then(Value::as_str) != Some("document") {
            continue;
        }

        let Some(info) = source.get("info") else {
            continue;
        };
        if !info.is_object() {
            return Err(unreadable());
        }

        let servers = mcp_servers(info, OpenCodeGeneration::V2).ok_or_else(unreadable)?;
        names.extend(servers.into_iter().map(|(name, _)| name));
    }

    Ok(names)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn opencode_2_names_come_from_every_document_source() {
        let printed = json!([
            { "type": "document", "path": "/h/.config/opencode/opencode.json", "info": { "mcp": {
                "servers": { "a": { "type": "local", "command": ["x"], "disabled": true } },
                "legacy": { "type": "remote", "url": "https://x.example.com" },
            } } },
            { "type": "directory", "path": "/h/.config/opencode" },
            { "type": "document", "info": { "mcp": { "servers": { "b": { "type": "remote", "url": "https://y" } } } } },
            { "type": "document", "info": { "model": "m" } },
        ]);

        let names = source_mcp_names(&printed.to_string()).unwrap();
        assert_eq!(names.into_iter().collect::<Vec<_>>(), ["a", "b", "legacy"]);

        let wrapped = json!({ "data": printed });
        assert_eq!(source_mcp_names(&wrapped.to_string()).unwrap().len(), 3);
    }

    #[test]
    fn anything_but_config_sources_is_unreadable() {
        for printed in [
            "",
            "not json",
            "{}",
            r#"{"mcp": {}}"#,
            r#"[{"type": "document", "info": []}]"#,
            r#"[{"type": "document", "info": {"mcp": []}}]"#,
            r#"[{"type": "document", "info": {"mcp": {"servers": 1}}}]"#,
        ] {
            assert!(source_mcp_names(printed).is_err(), "{printed:?}");
        }

        assert!(source_mcp_names("[]").unwrap().is_empty());
    }
}
