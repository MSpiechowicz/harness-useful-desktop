//! The OpenCode 2.1.2 traffic recorded with a free model (`fixtures/opencode2/recorded/*.jsonl`):
//! one JSON row per HTTP exchange (`{kind: "http", method, path, body, status, response}`) or SSE
//! event (`{kind: "sse", event}`). Paths name the project directory `<PROJ>`; the server answered
//! with `/work/proj`. Long texts are cut and repeated events thinned; shapes are as recorded.

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

/// The recorded project directory.
pub const DIRECTORY: &str = "/work/proj";

/// One recorded HTTP exchange.
#[derive(Debug, Clone)]
pub struct Exchange {
    /// The recording it comes from.
    pub file: String,
    /// `GET`, `POST`, …
    pub method: String,
    /// Path and query, with the directory percent-encoded as sent.
    pub target: String,
    /// The JSON request body (`null` for none).
    pub body: Value,
    /// The status code.
    pub status: u16,
    /// The JSON response (`null` for none).
    pub response: Value,
}

impl Exchange {
    /// The path without the query.
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }
}

/// Every recording: its file name and rows.
pub fn recordings() -> Vec<(String, Vec<Value>)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/opencode2/recorded");
    let mut files: Vec<_> = fs::read_dir(&dir)
        .expect("recordings directory")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    files.sort();

    files
        .into_iter()
        .map(|path| {
            let name = path.file_name().expect("name").to_string_lossy().into_owned();
            let rows = fs::read_to_string(&path)
                .expect("recording")
                .lines()
                .map(|line| serde_json::from_str(line).expect("row"))
                .collect();
            (name, rows)
        })
        .collect()
}

/// Every recorded HTTP exchange, in order.
pub fn exchanges() -> Vec<Exchange> {
    let directory = format!("location%5Bdirectory%5D={}", "%2Fwork%2Fproj");
    let mut exchanges = Vec::new();

    for (file, rows) in recordings() {
        for row in rows.into_iter().filter(|row| row["kind"] == "http") {
            exchanges.push(Exchange {
                file: file.clone(),
                method: row["method"].as_str().expect("method").to_owned(),
                target: row["path"]
                    .as_str()
                    .expect("path")
                    .replace("location%5Bdirectory%5D=<PROJ>", &directory),
                body: row["body"].clone(),
                status: u16::try_from(row["status"].as_u64().expect("status")).expect("status code"),
                response: row["response"].clone(),
            });
        }
    }

    exchanges
}

/// Every recorded SSE event's `data:` text, with its recording's name.
pub fn events() -> Vec<(String, String)> {
    recordings()
        .into_iter()
        .flat_map(|(file, rows)| {
            rows.into_iter()
                .filter(|row| row["kind"] == "sse")
                .map(move |row| (file.clone(), row["event"].to_string()))
        })
        .collect()
}

/// The recorded exchanges whose method and path (without query) match.
pub fn find(method: &str, path_suffix: &str) -> Vec<Exchange> {
    exchanges()
        .into_iter()
        .filter(|exchange| exchange.method == method && exchange.path().ends_with(path_suffix))
        .collect()
}
