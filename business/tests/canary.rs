//! Canary test: a fixture home hides `CANARY-7f3a9c` in deny-listed files and in every secret
//! config value. Reading the whole tree through `SafeReader` and redacting what it returns must
//! never surface the canary.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use harness_business::BusinessError;
use harness_business::model::ConfigValue;
use harness_business::security::{Redactor, SafeReader, is_denied, redact_text};

const CANARY: &str = "CANARY-7f3a9c";

fn canary_home() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canary-home")
}

/// Every file below `dir` (enumeration only; contents are read through `SafeReader`).
fn files_below(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];

    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }

    files.sort();
    files
}

fn relative(path: &Path) -> String {
    path.strip_prefix(canary_home())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

fn redact_config(path: &Path, text: &str) -> ConfigValue {
    let value: serde_json::Value = match path.extension().and_then(|extension| extension.to_str()) {
        Some("yml" | "yaml") => serde_norway::from_str(text).unwrap(),
        _ => serde_json::from_str(text).unwrap(),
    };

    Redactor.redact(&value)
}

#[test]
fn reading_and_redacting_the_fixture_home_never_reveals_the_canary() {
    let mut refused = BTreeSet::new();
    let mut exposed = String::new();

    for path in files_below(&canary_home()) {
        match SafeReader::read_to_string(&path, SafeReader::DEFAULT_MAX_BYTES) {
            Ok(text) => {
                let redacted = redact_config(&path, &text);
                exposed.push_str(&serde_json::to_string(&redacted).unwrap());
            }
            Err(BusinessError::Forbidden(_)) => {
                refused.insert(relative(&path));
            }
            Err(other) => panic!("unexpected error for {}: {other}", relative(&path)),
        }
    }

    let expected: BTreeSet<String> = [
        ".env",
        ".omp/agent/agent.db",
        ".omp/agent/blobs/blob-1.json",
        ".omp/agent/install-id",
        ".omp/agent/sessions/-proj/url-search/result.json",
        ".omp/agent/x.log",
        ".pi/agent/auth.json",
        ".pi/agent/mcp-auth.json",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    assert_eq!(refused, expected);
    assert!(!exposed.contains(CANARY), "canary leaked: {exposed}");

    // Positive control: non-secret configuration is still visible.
    for visible in [
        "anthropic/claude-sonnet-4-5",
        "@modelcontextprotocol/server-github",
        "desktop-client",
        "repo read:org",
        "qwen3-coder:30b",
        "\"4096\"",
        "region=eu",
        "CANARY_TOKEN_VAR",
    ] {
        assert!(exposed.contains(visible), "expected {visible} to stay visible");
    }
}

#[test]
fn every_deny_listed_fixture_is_recognized_without_touching_the_filesystem() {
    for name in ["agent.db", "auth.json", "mcp-auth.json", "x.log", "install-id"] {
        let path = canary_home().join(".omp/agent").join(name);
        assert!(is_denied(&path).is_some(), "{name} must be denied");
    }
}

#[test]
fn redacting_free_text_hides_canary_secrets_in_cli_output() {
    let stderr = format!(
        "fetch https://bot:{CANARY}@git.example.com/repo.git failed\n\
         OPENAI_API_KEY={CANARY} rejected; Authorization: Bearer {CANARY}-abcdef"
    );

    let redacted = redact_text(&stderr);

    assert!(!redacted.contains(CANARY), "canary leaked: {redacted}");
    assert!(redacted.contains("https://••••@git.example.com/repo.git"));
}
