//! A recording stand-in for the `claude` binary (the real one never runs in tests): every run is
//! recorded, and `claude mcp add-json|remove --scope user <name>` change the `mcpServers` of the
//! global file as Claude Code would (an existing name is refused with exit code 1). Other
//! commands print the answer given to [`FakeClaude::answer`], or succeed with no output.
//!
//! Each helper is used by some tests only, hence the `dead_code` allowance.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use harness_business::Result;
use harness_business::env::Env;
use harness_business::harness::{Harness, HarnessCli};
use harness_business::harnesses::Claude;
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner, ResolvedBinary};
use serde_json::{Map, Value, json};

/// The fake (see the module docs).
pub struct FakeClaude {
    /// Claude Code's global file the `mcp` commands change.
    global: PathBuf,
    seen: Mutex<Vec<ExecSpec>>,
    /// What runs other than `mcp` answer, in order (then: exit 0, no output).
    answers: Mutex<Vec<ExecOutput>>,
    /// `mcp` verbs whose next run fails (exit 1) without changing anything, one entry per run.
    failing: Mutex<Vec<String>>,
}

impl FakeClaude {
    pub fn new(global: &Path) -> Self {
        Self {
            global: global.to_path_buf(),
            seen: Mutex::new(Vec::new()),
            answers: Mutex::new(Vec::new()),
            failing: Mutex::new(Vec::new()),
        }
    }

    /// Makes the next run of `claude mcp <verb>` fail with exit code 1, changing nothing.
    pub fn fail(&self, verb: &str) {
        self.failing.lock().unwrap().push(verb.to_owned());
    }

    /// Queues the output of the next run that is not an `mcp` command.
    pub fn answer(&self, output: ExecOutput) {
        self.answers.lock().unwrap().push(output);
    }

    /// The runs so far.
    pub fn specs(&self) -> Vec<ExecSpec> {
        self.seen.lock().unwrap().clone()
    }

    /// The arguments of each run so far.
    pub fn argvs(&self) -> Vec<Vec<String>> {
        self.specs().iter().map(args).collect()
    }

    fn mcp(&self, args: &[String]) -> ExecOutput {
        let verb = args.get(1).map(String::as_str).unwrap_or_default();

        let mut failing = self.failing.lock().unwrap();
        if let Some(index) = failing.iter().position(|failed| failed == verb) {
            failing.remove(index);
            return exit(1, "", "Error: failed");
        }
        drop(failing);

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

impl ProcessRunner for FakeClaude {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        self.seen.lock().unwrap().push(spec.clone());
        let args = args(spec);

        let output = if args.first().map(String::as_str) == Some("mcp") {
            self.mcp(&args)
        } else {
            let mut answers = self.answers.lock().unwrap();
            if answers.is_empty() {
                exit(0, "", "")
            } else {
                answers.remove(0)
            }
        };

        Box::pin(async move { Ok(output) })
    }
}

/// The arguments of `spec`, as text.
pub fn args(spec: &ExecSpec) -> Vec<String> {
    spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

/// A finished run.
pub fn exit(code: i32, stdout: &str, stderr: &str) -> ExecOutput {
    ExecOutput {
        status: Some(code),
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
        ..ExecOutput::default()
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
