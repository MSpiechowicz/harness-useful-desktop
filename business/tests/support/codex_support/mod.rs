//! A recording stand-in for the `codex` binary (the real one never runs in tests): every run is
//! recorded, and `codex mcp add <name> -- <command> <args…>`, `codex mcp add <name> --url <url>
//! [--bearer-token-env-var <name>]` and `codex mcp remove <name>` change the `mcp_servers` of its
//! `config.toml` (an add replaces a server of the same name, which the app must never rely on).
//! The file is written back as plain TOML, so comments are not kept. Other commands print the
//! answer given to [`FakeCodex::answer`], or succeed with no output.
//!
//! Each helper is used by some tests only, hence the `dead_code` allowance.
#![allow(dead_code)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use harness_business::Result;
use harness_business::env::Env;
use harness_business::harness::{Harness, HarnessCli};
use harness_business::harnesses::Codex;
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner, ResolvedBinary};
use serde_json::{Map, Value, json};

use crate::recording_cli::RecordingCli;
pub use crate::recording_cli::exit;

/// The fake (see the module docs).
pub struct FakeCodex {
    /// The `config.toml` the `mcp` commands change.
    config: PathBuf,
    recording: RecordingCli,
}

impl FakeCodex {
    pub fn new(config: &Path) -> Self {
        Self {
            config: config.to_path_buf(),
            recording: RecordingCli::default(),
        }
    }

    fn mcp(&self, args: &[String]) -> ExecOutput {
        let mut document = read_toml(&self.config);
        let servers = document
            .entry("mcp_servers")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .unwrap();

        match &args[1..] {
            [add, name, separator, command, words @ ..] if add == "add" && separator == "--" => {
                servers.insert(name.clone(), json!({ "command": command, "args": words }));
            }
            [add, name, flag, url] if add == "add" && flag == "--url" => {
                servers.insert(name.clone(), json!({ "url": url }));
            }
            [add, name, flag, url, token, variable]
                if add == "add" && flag == "--url" && token == "--bearer-token-env-var" =>
            {
                servers.insert(name.clone(), json!({ "url": url, "bearer_token_env_var": variable }));
            }
            [remove, name] if remove == "remove" => {
                if servers.shift_remove(name).is_none() {
                    return exit(1, "", &format!("No MCP server named '{name}' found."));
                }
            }
            _ => return exit(1, "", "unexpected arguments"),
        }

        write_toml(&self.config, &document);
        exit(0, "", "")
    }
}

/// `fail`, `answer`, `specs` and `argvs` of the recording (see [`RecordingCli`]).
impl std::ops::Deref for FakeCodex {
    type Target = RecordingCli;

    fn deref(&self) -> &RecordingCli {
        &self.recording
    }
}

impl ProcessRunner for FakeCodex {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let output = self.recording.run(spec, |args| self.mcp(args));

        Box::pin(async move { Ok(output) })
    }
}

/// The file's TOML as a JSON object, or an empty one.
pub fn read_toml(path: &Path) -> Map<String, Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<Value>(&text).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

/// Writes `document` (scalars, lists of scalars and nested tables) to `path` as TOML, creating its
/// directory.
pub fn write_toml(path: &Path, document: &Map<String, Value>) {
    let mut text = String::new();
    write_table(&mut text, &[], document);

    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn write_table(text: &mut String, path: &[String], table: &Map<String, Value>) {
    let (tables, values): (Vec<_>, Vec<_>) = table.iter().partition(|(_, value)| value.is_object());

    if !path.is_empty() {
        let header: Vec<String> = path.iter().map(|key| key_text(key)).collect();
        writeln!(text, "\n[{}]", header.join(".")).unwrap();
    }

    for (key, value) in values {
        writeln!(text, "{} = {}", key_text(key), value_text(value)).unwrap();
    }

    for (key, value) in tables {
        let mut child = path.to_vec();
        child.push(key.clone());
        write_table(text, &child, value.as_object().unwrap());
    }
}

fn key_text(key: &str) -> String {
    let bare = !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');

    if bare {
        key.to_owned()
    } else {
        Value::String(key.to_owned()).to_string()
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::Array(items) => format!("[{}]", items.iter().map(value_text).collect::<Vec<_>>().join(", ")),
        other => other.to_string(),
    }
}

/// Writes `value` (a JSON object) to `path` as TOML.
pub fn write_config(path: &Path, value: &Value) {
    write_toml(path, value.as_object().expect("a table"));
}

/// The detected `codex` of the tests (a path that is never run).
pub fn binary(home: &Path) -> ResolvedBinary {
    ResolvedBinary {
        path: home.join(".local/bin/codex"),
        is_batch_script: false,
    }
}

/// The CLI of [`Codex`] for `runner`.
pub fn cli<'a>(env: &'a Env, runner: &'a FakeCodex, binary: &'a ResolvedBinary) -> HarnessCli<'a> {
    HarnessCli {
        harness: &Codex as &dyn Harness,
        env,
        runner,
        binary,
    }
}

/// `{"command": <command>, "args": ["mcp"]}`.
pub fn stdio(command: &str) -> Value {
    json!({ "command": command, "args": ["mcp"] })
}

/// `codex mcp add <name> -- <command> mcp`.
pub fn add_stdio(name: &str, command: &str) -> Vec<String> {
    ["mcp", "add", name, "--", command, "mcp"].map(str::to_owned).to_vec()
}

/// `codex mcp remove <name>`.
pub fn remove(name: &str) -> Vec<String> {
    ["mcp", "remove", name].map(str::to_owned).to_vec()
}
