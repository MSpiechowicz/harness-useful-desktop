//! What the scripted CLIs (`claude`, `codex`) share: the [`Player`] runs the steps every dialect
//! has (`respond`, `emit`, `replay`, output and end steps) and leaves `expect` and the answers to
//! the [`Dialect`]; a `replay` step writes the CLI's frames of a recording, every written frame
//! has the recorded project directory at the start of a string moved to the working directory,
//! and the app's frames wait in an [`Inbox`] until an `expect` takes them.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use serde_json::{Value, json};

use crate::{fail, run_end_step, run_output_step, spawn_stdin_reader, substitute};

/// What a scripted CLI does its own way.
pub trait Dialect {
    /// A frame's kind, as a `replay` step's `except` names it.
    fn kind(frame: &Value) -> Option<String>;

    /// The `$index` of the n-th frame an `emit` with `times` writes.
    fn index(index: u64) -> Value;

    /// An `expect` step of `kind`.
    fn expect(player: &mut Player, kind: &str, step: &Value);

    /// Answers the request `id` with the step's `reply` (or `error`).
    fn answer(player: &Player, id: &Value, step: &Value);
}

/// Plays a scenario.
pub struct Player {
    pub inbox: Inbox,
    /// Values captured by `expect` steps, by name.
    pub captured: HashMap<String, Value>,
    pub recordings: Recordings,
}

impl Player {
    /// Plays `steps` of a CLI that wrote the rows `written` of recordings in `project`, then waits
    /// for stdin EOF and exits 0.
    pub fn play<D: Dialect>(steps: &[Value], scenario: &Path, project: &'static str, written: &'static str) -> ! {
        let mut player = Self {
            inbox: Inbox::new(),
            captured: HashMap::new(),
            recordings: Recordings::new(scenario, project, written),
        };

        for step in steps {
            player.run::<D>(step);
        }

        player.inbox.finish();
    }

    fn run<D: Dialect>(&mut self, step: &Value) {
        if run_output_step(step) {
            return;
        }

        let field = |name: &str| step.get(name);

        if let Some(kind) = field("expect").and_then(Value::as_str) {
            D::expect(self, kind, step);
        } else if let Some(name) = field("respond").and_then(Value::as_str) {
            let id = self.captured.get(name).cloned().unwrap_or(Value::Null);
            D::answer(self, &id, step);
        } else if let Some(frame) = field("emit") {
            let times = field("times").and_then(Value::as_u64).unwrap_or(1);

            for index in 0..times {
                self.captured.insert("index".to_owned(), D::index(index));
                let frame = self.substitute(frame);
                write(&self.recordings.recorded(&frame, &[]));
            }
        } else if field("replay").is_some() {
            for frame in self.recordings.replayed(step, &|value| self.substitute(value), D::kind) {
                write(&frame);
            }
        } else {
            run_end_step(step);
        }
    }

    /// Replaces `"$name"` strings with captured values.
    pub fn substitute(&self, value: &Value) -> Value {
        substitute(value, &|name| self.captured.get(name).cloned())
    }
}

/// The app's frames: read from stdin, kept until an `expect` takes them.
pub struct Inbox {
    stdin: Receiver<Value>,
    unclaimed: Vec<Value>,
}

impl Inbox {
    pub fn new() -> Self {
        Self {
            stdin: spawn_stdin_reader(),
            unclaimed: Vec::new(),
        }
    }

    /// The next frame `wanted` takes (frames it does not take are kept for later); exits quietly
    /// when stdin closes first.
    pub fn next(&mut self, wanted: impl Fn(&Value) -> bool) -> Value {
        if let Some(index) = self.unclaimed.iter().position(&wanted) {
            return self.unclaimed.remove(index);
        }

        loop {
            let Ok(frame) = self.stdin.recv() else {
                std::process::exit(0);
            };

            if wanted(&frame) {
                return frame;
            }

            self.unclaimed.push(frame);
        }
    }

    /// Waits for stdin EOF and exits 0, as the CLIs do after their last output.
    pub fn finish(&self) -> ! {
        while self.stdin.recv().is_ok() {}
        std::process::exit(0);
    }
}

/// Writes one frame and a newline; exits once stdout is gone.
pub fn write(frame: &Value) {
    let mut line = serde_json::to_vec(frame).unwrap_or_default();
    line.push(b'\n');

    let mut stdout = std::io::stdout().lock();
    if stdout.write_all(&line).and_then(|()| stdout.flush()).is_err() {
        std::process::exit(0);
    }
}

/// Where a scripted CLI finds its recordings and what it moves in them.
pub struct Recordings {
    /// The scenario's folder (recordings are relative to it).
    dir: PathBuf,
    /// The project directory of the recordings.
    project: &'static str,
    /// The `dir` of the rows the CLI wrote.
    written: &'static str,
    cwd: String,
}

impl Recordings {
    pub fn new(scenario: &Path, project: &'static str, written: &'static str) -> Self {
        Self {
            dir: scenario.parent().map(Path::to_path_buf).unwrap_or_default(),
            project,
            written,
            cwd: std::env::current_dir()
                .map(|dir| dir.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }

    /// The frames a `replay` step writes: the CLI's rows of its recording from line `from` to
    /// line `to` (1-based, both included), without those whose `kind` is in `except`, with the
    /// step's `replace` (values through `substitute`) and the working directory applied.
    pub fn replayed(
        &self,
        step: &Value,
        substitute: &dyn Fn(&Value) -> Value,
        kind: fn(&Value) -> Option<String>,
    ) -> Vec<Value> {
        let file = step.get("replay").and_then(Value::as_str).unwrap_or_default();
        let path = self.dir.join(file);
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|_| fail(2, &format!("cannot read {}", path.display())));

        let line = |name: &str| step.get(name).and_then(Value::as_u64).map(|line| line as usize);
        let (from, to) = (line("from").unwrap_or(1), line("to").unwrap_or(usize::MAX));
        let except: Vec<&str> = step
            .get("except")
            .and_then(Value::as_array)
            .map(|kinds| kinds.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();

        let replace: Vec<(Value, Value)> = step
            .get("replace")
            .and_then(Value::as_object)
            .map(|pairs| {
                pairs
                    .iter()
                    .map(|(recorded, value)| (json!(recorded), substitute(value)))
                    .collect()
            })
            .unwrap_or_default();

        let mut frames = Vec::new();

        for (index, row) in text.lines().enumerate() {
            let number = index + 1;
            if number < from || number > to || row.trim().is_empty() {
                continue;
            }

            let row: Value =
                serde_json::from_str(row).unwrap_or_else(|_| fail(2, &format!("bad row {number} of {file}")));
            let frame = &row["frame"];
            let left_out = kind(frame).is_some_and(|kind| except.contains(&kind.as_str()));

            if row["dir"] == self.written && !left_out {
                frames.push(self.recorded(frame, &replace));
            }
        }

        frames
    }

    /// A recorded frame with `replace` and the working directory applied.
    pub fn recorded(&self, value: &Value, replace: &[(Value, Value)]) -> Value {
        if let Some((_, value)) = replace.iter().find(|(recorded, _)| recorded == value) {
            return value.clone();
        }

        match value {
            Value::String(text) => match text.strip_prefix(self.project) {
                Some(rest) => Value::String(format!("{}{rest}", self.cwd)),
                None => value.clone(),
            },
            Value::Array(items) => Value::Array(items.iter().map(|item| self.recorded(item, replace)).collect()),
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(key, item)| (key.clone(), self.recorded(item, replace)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }
}
