//! Helpers shared by the adapter test crates: synthetic fixture homes, the demo project and
//! machine-independent path rewriting.
//!
//! Each test crate uses a different subset, hence the `dead_code` allowance.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use harness_business::env::Env;
use serde::Serialize;
use serde_json::Value;

/// Hidden in every secret value and every deny-listed file of the fixture homes.
pub const CANARY: &str = "CANARY-7f3a9c";

/// The synthetic home every exported path is rewritten to.
pub const DEMO_HOME: &str = "/home/demo";

/// The synthetic project path every exported project path is rewritten to.
pub const DEMO_PROJECT: &str = "/home/demo/Projects/demo";

fn fixtures_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    dunce::canonicalize(dir).expect("fixtures directory exists")
}

/// `tests/fixtures/omp-home` (canonical).
pub fn omp_home() -> PathBuf {
    fixtures_dir().join("omp-home")
}

/// `tests/fixtures/pi-home` (canonical).
pub fn pi_home() -> PathBuf {
    fixtures_dir().join("pi-home")
}

/// `tests/fixtures/opencode-home` (canonical).
pub fn opencode_home() -> PathBuf {
    fixtures_dir().join("opencode-home")
}

/// `tests/fixtures/claude-home` (canonical).
pub fn claude_home() -> PathBuf {
    fixtures_dir().join("claude-home")
}

/// `tests/fixtures/projects/demo` (canonical), marked as a repository root.
///
/// Git cannot track a `.git` entry, so the marker directory is created at run time; without it the
/// ancestor walk would continue into this repository.
pub fn demo_project() -> PathBuf {
    let project = fixtures_dir().join("projects/demo");
    std::fs::create_dir_all(project.join(".git")).expect("create .git marker");

    project
}

/// An environment rooted at `home` with no variables (no profile, no XDG, no PATH).
pub fn env_at(home: &Path) -> Env {
    Env::for_tests(home)
}

/// Serializes `value` and rewrites fixture paths to `/home/demo/...` (see [`DEMO_HOME`]).
pub fn exported(value: impl Serialize, home: &Path, project: &Path) -> Value {
    let value = serde_json::to_value(value).expect("DTOs always serialize");
    let replacements = [
        (project.to_string_lossy().into_owned(), DEMO_PROJECT),
        (home.to_string_lossy().into_owned(), DEMO_HOME),
    ];

    rewrite(value, &replacements)
}

fn rewrite(value: Value, replacements: &[(String, &str)]) -> Value {
    match value {
        Value::String(text) => Value::String(rewrite_text(text, replacements)),
        Value::Array(items) => Value::Array(items.into_iter().map(|item| rewrite(item, replacements)).collect()),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, item)| (key, rewrite(item, replacements)))
                .collect(),
        ),
        other => other,
    }
}

/// Replaces the first matching prefix; rewritten paths use `/` so Windows output matches too.
fn rewrite_text(text: String, replacements: &[(String, &str)]) -> String {
    for (from, to) in replacements {
        if let Some(rest) = text.strip_prefix(from.as_str()) {
            return format!("{to}{}", rest.replace('\\', "/"));
        }
    }

    text
}

/// The serialized JSON of `value`, for substring checks.
pub fn json_text(value: impl Serialize) -> String {
    serde_json::to_string(&value).expect("DTOs always serialize")
}

/// Sets the Unix permission bits of `path`; nothing to do on other systems.
pub fn set_mode(path: &Path, mode: u32) {
    #[cfg(unix)]
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(mode)).expect("set the mode");

    #[cfg(not(unix))]
    let _ = (path, mode);
}

/// Writes `content` to `path`, creating its parent directories.
pub fn write_file(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// `count` directories under `root`, each holding `per_dir` empty files ending in `suffix`; their
/// paths, for a settings list that is more than a scan budget allows.
pub fn big_dirs(root: &Path, count: usize, per_dir: usize, suffix: &str) -> Vec<String> {
    (0..count)
        .map(|index| {
            let dir = root.join(format!("d{index:02}"));
            std::fs::create_dir_all(&dir).unwrap();
            for file in 0..per_dir {
                std::fs::write(dir.join(format!("p{file:04}{suffix}")), "").unwrap();
            }

            dir.to_string_lossy().into_owned()
        })
        .collect()
}

/// Runs `work` and fails when it takes `limit_secs` seconds or more: a crafted tree must not stall a view.
pub fn within<T>(limit_secs: f32, work: impl FnOnce() -> T) -> T {
    let started = std::time::Instant::now();
    let result = work();
    let elapsed = started.elapsed();
    assert!(elapsed.as_secs_f32() < limit_secs, "{elapsed:?}");

    result
}
