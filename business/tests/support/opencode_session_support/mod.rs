//! A replayed OpenCode CLI for the CLI-session tests: the fixtures in
//! `tests/fixtures/sessions/opencode` are real `opencode session list --format json` and
//! `opencode export` output (OpenCode 1.19.3, sessions imported with `opencode import` into a
//! scratch home), with the project directory replaced by `{{PROJECT}}`.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Barrier, Mutex};
use std::time::Duration;

use harness_business::harness::CliSessionCtx;
use harness_business::model::{HarnessId, IpcErrorCode, SessionRef};
use harness_business::process::{BlockingRunner, ExecOutput, ExecSpec, ResolvedBinary};

use crate::session_support::{SessionHome, session, session_home};

/// The session with text, reasoning, tool calls, an attachment and a subtask.
pub const MAIN: &str = "ses_2b7f3c9a1ffeA1b2C3d4E5f6G7";

/// The session started in `<project>/sub`.
pub const SUB: &str = "ses_2b7f3c9a2000Z9y8X7w6V5u4T3";

/// The argv of `session list`.
pub const LIST_ARGS: [&str; 5] = ["session", "list", "--pure", "--format", "json"];

/// Records every spec and answers with queued outputs; a spawn without a queued output fails the
/// test.
#[derive(Default)]
pub struct Replay {
    specs: Mutex<Vec<ExecSpec>>,
    outputs: Mutex<VecDeque<ExecOutput>>,
}

impl Replay {
    pub fn answering(outputs: impl IntoIterator<Item = ExecOutput>) -> Self {
        Self {
            specs: Mutex::default(),
            outputs: Mutex::new(outputs.into_iter().collect()),
        }
    }

    pub fn queue(&self, output: ExecOutput) {
        self.outputs.lock().unwrap().push_back(output);
    }

    pub fn specs(&self) -> Vec<ExecSpec> {
        self.specs.lock().unwrap().clone()
    }

    pub fn argv(&self) -> Vec<Vec<String>> {
        self.specs().iter().map(argv).collect()
    }
}

impl BlockingRunner for Replay {
    fn run(&self, spec: &ExecSpec) -> harness_business::Result<ExecOutput> {
        self.specs.lock().unwrap().push(spec.clone());

        let output = self.outputs.lock().unwrap().pop_front();
        Ok(output.unwrap_or_else(|| panic!("unexpected spawn: {:?}", argv(spec))))
    }
}

/// A [`Replay`] that holds one run (counting from 0) until [`Held::release`]; [`Held::wait_held`]
/// returns once that run started.
pub struct Held {
    replay: Replay,
    hold: usize,
    spawned: AtomicUsize,
    started: Barrier,
    released: Barrier,
}

impl Held {
    pub fn answering(hold: usize, outputs: impl IntoIterator<Item = ExecOutput>) -> Self {
        Self {
            replay: Replay::answering(outputs),
            hold,
            spawned: AtomicUsize::new(0),
            started: Barrier::new(2),
            released: Barrier::new(2),
        }
    }

    pub fn wait_held(&self) {
        self.started.wait();
    }

    pub fn release(&self) {
        self.released.wait();
    }

    pub fn argv(&self) -> Vec<Vec<String>> {
        self.replay.argv()
    }
}

impl BlockingRunner for Held {
    fn run(&self, spec: &ExecSpec) -> harness_business::Result<ExecOutput> {
        let output = self.replay.run(spec);

        if self.spawned.fetch_add(1, Ordering::SeqCst) == self.hold {
            self.started.wait();
            self.released.wait();
        }

        output
    }
}

/// Answers every spawn with the same output after a delay, counting the runs and the most that
/// were in flight at once.
pub struct Slow {
    output: ExecOutput,
    delay: Duration,
    runs: AtomicUsize,
    running: AtomicUsize,
    peak: AtomicUsize,
}

impl Slow {
    pub fn new(output: ExecOutput, delay: Duration) -> Self {
        Self {
            output,
            delay,
            runs: AtomicUsize::new(0),
            running: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        }
    }

    pub fn runs(&self) -> usize {
        self.runs.load(Ordering::SeqCst)
    }

    pub fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }
}

impl BlockingRunner for Slow {
    fn run(&self, _spec: &ExecSpec) -> harness_business::Result<ExecOutput> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let running = self.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(running, Ordering::SeqCst);

        std::thread::sleep(self.delay);

        self.running.fetch_sub(1, Ordering::SeqCst);
        Ok(self.output.clone())
    }
}

pub fn argv(spec: &ExecSpec) -> Vec<String> {
    spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

pub fn ok(stdout: impl Into<String>) -> ExecOutput {
    ExecOutput {
        status: Some(0),
        stdout: stdout.into(),
        ..ExecOutput::default()
    }
}

pub fn failed(stderr: &str) -> ExecOutput {
    ExecOutput {
        status: Some(1),
        stderr: stderr.to_owned(),
        ..ExecOutput::default()
    }
}

/// A temporary home whose demo project has `src/`, `notes/` and `sub/`.
pub struct Fixture {
    pub home: SessionHome,
    pub binary: ResolvedBinary,
}

impl Fixture {
    pub fn new() -> Self {
        let home = session_home();
        fs::create_dir_all(home.project.join("notes")).unwrap();
        fs::create_dir_all(home.project.join("sub")).unwrap();

        let binary = ResolvedBinary {
            path: home.home.join(".opencode/bin/opencode"),
            is_batch_script: false,
        };

        Self { home, binary }
    }

    pub fn ctx<'a>(&'a self, project: &'a Path, runner: &'a dyn BlockingRunner) -> CliSessionCtx<'a> {
        CliSessionCtx {
            env: &self.home.env,
            project,
            binary: &self.binary,
            runner,
        }
    }

    pub fn project(&self) -> &Path {
        &self.home.project
    }

    /// A recorded fixture with `{{PROJECT}}` replaced by `project`.
    pub fn recorded(&self, name: &str, project: &Path) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sessions/opencode")
            .join(name);
        let text = fs::read_to_string(path).expect("fixture");

        // The path as it appears inside a JSON string.
        let quoted = serde_json::to_string(&project.to_string_lossy()).unwrap();
        text.replace("{{PROJECT}}", quoted.trim_matches('"'))
    }

    pub fn list_output(&self) -> ExecOutput {
        ok(self.recorded("session-list.json", self.project()))
    }

    pub fn export_output(&self) -> ExecOutput {
        ok(self.recorded("export-main.json", self.project()))
    }
}

pub fn opencode(id: &str) -> SessionRef {
    session(HarnessId::OpenCode, id)
}

pub fn error_code<T: std::fmt::Debug>(result: harness_business::Result<T>) -> IpcErrorCode {
    result.expect_err("expected an error").code()
}
