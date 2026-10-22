//! The Claude Code 2.1.304 stream-json traffic recorded on a subscription login
//! (`fixtures/claude/stream/*.jsonl`): one row per frame, `{t, dir, frame}`, where `dir` is `in`
//! (app → CLI), `out` (CLI → app), `err` (stderr) or `note` (the recorder's own remarks: argv,
//! timeouts, exit). The project directory is `/home/user/project`; accounts, paths and long lists
//! are scrubbed.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The recorded project directory.
pub const PROJECT: &str = "/home/user/project";

/// The JSON text of `frame` with the recorded project directory moved to `project`, written as a
/// JSON string so a Windows path keeps its backslashes escaped.
pub fn moved(frame: &Value, project: &Path) -> String {
    let quoted = serde_json::to_string(&project.to_string_lossy()).expect("a path as JSON");

    frame.to_string().replace(PROJECT, &quoted[1..quoted.len() - 1])
}

/// Every recording.
pub const FILES: [&str; 12] = [
    "bash.jsonl",
    "basic.jsonl",
    "clone.jsonl",
    "compact.jsonl",
    "controls.jsonl",
    "fork_at.jsonl",
    "interrupt.jsonl",
    "nologin.jsonl",
    "question.jsonl",
    "queue2.jsonl",
    "tools.jsonl",
    "utility.jsonl",
];

/// Which way a recorded frame went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    /// App → CLI.
    In,
    /// CLI → app.
    Out,
    /// Stderr or a recorder note.
    Other,
}

/// One recorded row.
#[derive(Debug, Clone)]
pub struct Row {
    pub dir: Dir,
    pub frame: Value,
}

/// The rows of recording `file`.
pub fn rows(file: &str) -> Vec<Row> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude/stream")
        .join(file);
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));

    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let row: Value = serde_json::from_str(line).unwrap_or_else(|error| panic!("{file}: {error}"));
            let dir = match row["dir"].as_str() {
                Some("in") => Dir::In,
                Some("out") => Dir::Out,
                _ => Dir::Other,
            };

            Row {
                dir,
                frame: row["frame"].clone(),
            }
        })
        .collect()
}

/// The frames of recording `file` that went `dir`.
pub fn frames(file: &str, dir: Dir) -> Vec<Value> {
    rows(file)
        .into_iter()
        .filter(|row| row.dir == dir)
        .map(|row| row.frame)
        .collect()
}

/// The `in` control request of `file` with `subtype`, the n-th one (0-based).
pub fn sent_request(file: &str, subtype: &str, nth: usize) -> Value {
    frames(file, Dir::In)
        .into_iter()
        .filter(|frame| frame["type"] == "control_request" && frame["request"]["subtype"] == subtype)
        .nth(nth)
        .unwrap_or_else(|| panic!("{file}: no {subtype} #{nth}"))
}

/// The `in` frames of `file` of `kind` (`user`, `control_response`, …).
pub fn sent(file: &str, kind: &str) -> Vec<Value> {
    frames(file, Dir::In)
        .into_iter()
        .filter(|frame| frame["type"] == kind)
        .collect()
}
