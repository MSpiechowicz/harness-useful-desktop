//! A recording stand-in for the `claude` binary (the real one never runs in tests): every run is
//! recorded, and `claude mcp add-json|remove --scope user <name>` change the `mcpServers` of the
//! global file as Claude Code would (an existing name is refused with exit code 1). Other
//! commands print the answer given to [`FakeClaude::answer`], or succeed with no output.
//!
//! Each helper is used by some tests only, hence the `dead_code` allowance.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use harness_business::Result;
use harness_business::env::Env;
use harness_business::harness::{Harness, HarnessCli};
use harness_business::harnesses::Claude;
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner, ResolvedBinary};
use serde_json::{Map, Value, json};

// `args` is used by the views binary only.
use crate::recording_cli::RecordingCli;
#[allow(unused_imports)]
pub use crate::recording_cli::{args, exit};

/// The fake (see the module docs).
pub struct FakeClaude {
    /// Claude Code's global file the `mcp` commands change.
    global: PathBuf,
    recording: RecordingCli,
}

impl FakeClaude {
    pub fn new(global: &Path) -> Self {
        Self {
            global: global.to_path_buf(),
            recording: RecordingCli::default(),
        }
    }

    fn mcp(&self, args: &[String]) -> ExecOutput {
        let verb = args.get(1).map(String::as_str).unwrap_or_default();
        let mut document = read_object(&self.global);
        let servers = document
            .entry("mcpServers")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .unwrap();

        match (verb, &args[2..]) {
            ("add-json", [scope, user, name, definition]) if scope == "--scope" && user == "user" => {
                if servers.contains_key(name) {
                    return exit(1, "", "MCP server already exists in user config");
                }

                servers.insert(name.clone(), serde_json::from_str(definition).unwrap());
            }
            ("remove", [scope, user, name]) if scope == "--scope" && user == "user" => {
                if servers.shift_remove(name).is_none() {
                    return exit(1, "", "No user-scoped MCP server found");
                }
            }
            _ => return exit(1, "", "unexpected arguments"),
        }

        std::fs::write(&self.global, Value::Object(document).to_string()).unwrap();
        exit(0, "", "")
    }
}

/// `fail`, `answer`, `specs` and `argvs` of the recording (see [`RecordingCli`]).
impl std::ops::Deref for FakeClaude {
    type Target = RecordingCli;

    fn deref(&self) -> &RecordingCli {
        &self.recording
    }
}

impl ProcessRunner for FakeClaude {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let output = self.recording.run(spec, |args| self.mcp(args));

        Box::pin(async move { Ok(output) })
    }
}

/// The file's JSON object, or an empty one.
pub fn read_object(path: &Path) -> Map<String, Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

/// Writes `value` to `path`, creating its directory.
pub fn write_json(path: &Path, value: &Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, value.to_string()).unwrap();
}

/// The detected `claude` of the tests (a path that is never run).
pub fn binary(home: &Path) -> ResolvedBinary {
    ResolvedBinary {
        path: home.join(".local/bin/claude"),
        is_batch_script: false,
    }
}

/// The CLI of [`Claude`] for `runner`.
pub fn cli<'a>(env: &'a Env, runner: &'a FakeClaude, binary: &'a ResolvedBinary) -> HarnessCli<'a> {
    HarnessCli {
        harness: &Claude as &dyn Harness,
        env,
        runner,
        binary,
    }
}

/// `{"type": "stdio", "command": <command>, "args": ["mcp"]}`.
pub fn stdio(command: &str) -> Value {
    json!({ "type": "stdio", "command": command, "args": ["mcp"] })
}
