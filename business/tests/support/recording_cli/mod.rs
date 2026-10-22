//! The recording core of the harness CLI stand-ins (`claude_support`, `codex_support`): every run
//! is recorded, an `mcp` run is handed to the stand-in (unless told to fail), and any other run
//! prints the answer given to [`RecordingCli::answer`], or succeeds with no output.
//!
//! Each helper is used by some tests only, hence the `dead_code` allowance.
#![allow(dead_code)]

use std::sync::Mutex;

use harness_business::process::{ExecOutput, ExecSpec};

/// See the module docs.
#[derive(Default)]
pub struct RecordingCli {
    seen: Mutex<Vec<ExecSpec>>,
    /// What runs other than `mcp` answer, in order (then: exit 0, no output).
    answers: Mutex<Vec<ExecOutput>>,
    /// `mcp` verbs whose next run fails (exit 1) without changing anything, one entry per run.
    failing: Mutex<Vec<String>>,
}

impl RecordingCli {
    /// Makes the next run of `mcp <verb>` fail with exit code 1, changing nothing.
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

    /// Records `spec` and answers it: an `mcp` run through `mcp` (its arguments), unless its verb
    /// was told to fail.
    pub fn run(&self, spec: &ExecSpec, mcp: impl FnOnce(&[String]) -> ExecOutput) -> ExecOutput {
        self.seen.lock().unwrap().push(spec.clone());
        let args = args(spec);

        if args.first().map(String::as_str) != Some("mcp") {
            let mut answers = self.answers.lock().unwrap();

            return if answers.is_empty() {
                exit(0, "", "")
            } else {
                answers.remove(0)
            };
        }

        let verb = args.get(1).map(String::as_str).unwrap_or_default();
        let mut failing = self.failing.lock().unwrap();

        if let Some(index) = failing.iter().position(|failed| failed == verb) {
            failing.remove(index);
            return exit(1, "", "Error: failed");
        }

        drop(failing);
        mcp(&args)
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
