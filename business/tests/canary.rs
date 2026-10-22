//! Canary test: a fixture home hides `CANARY-7f3a9c` in deny-listed files and in every secret
//! config value. Reading the whole tree through `SafeReader` and redacting what it returns must
//! never surface the canary.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use common::files_below;

use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::model::{ConfigValue, SecretKind};
use harness_business::security::{RedactionContext, Redactor, SafeReader, is_denied, redact_text};
use serde_json::json;

const CANARY: &str = "CANARY-7f3a9c";

fn canary_home() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canary-home")
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

/// `tests/fixtures/claude-home`: a synthetic Claude Code home whose private files all hide the
/// canary, with a second config directory (`claude-work`) for `CLAUDE_CONFIG_DIR`.
fn claude_home() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-home")
}

/// The files of the Claude fixture home that stay readable: the global files, settings, managed
/// settings and memory files. Their canaries sit in values the views hide or never show (see
/// `adapter_canary.rs`).
const CLAUDE_READABLE: &[&str] = &[
    ".claude.json",
    ".claude/CLAUDE.md",
    ".claude/settings.json",
    "Program Files/ClaudeCode/managed-settings.d/.draft.json",
    "Program Files/ClaudeCode/managed-settings.d/10-team.json",
    "Program Files/ClaudeCode/managed-settings.d/README.txt",
    "Program Files/ClaudeCode/managed-settings.json",
    "claude-work/.claude.json",
    "claude-work/settings.json",
];

#[test]
fn every_private_claude_file_is_refused_and_only_its_settings_stay_readable() {
    let home = claude_home();
    // An environment naming `claude-work` as the config directory covers its private files too.
    let _env = Env::for_tests(&home).with_var("CLAUDE_CONFIG_DIR", home.join("claude-work").to_string_lossy());
    let mut readable = BTreeSet::new();

    for path in files_below(&home) {
        let name = path.strip_prefix(&home).unwrap().to_string_lossy().replace('\\', "/");

        match SafeReader::read_to_string(&path, SafeReader::DEFAULT_MAX_BYTES) {
            Ok(_) => {
                readable.insert(name);
            }
            Err(BusinessError::Forbidden(_)) => {
                assert!(
                    is_denied(&path).is_some(),
                    "{name} is refused without touching the filesystem"
                );
            }
            Err(other) => panic!("unexpected error for {name}: {other}"),
        }
    }

    let expected: BTreeSet<String> = CLAUDE_READABLE.iter().copied().map(String::from).collect();
    assert_eq!(readable, expected);
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

/// The value at `path` inside a redacted map.
fn field<'a>(value: &'a ConfigValue, path: &[&str]) -> &'a ConfigValue {
    path.iter().fold(value, |current, key| {
        let ConfigValue::Map(fields) = current else {
            panic!("expected a map at {key}");
        };

        &fields.iter().find(|field| field.key == *key).unwrap().value
    })
}

#[test]
fn redacting_an_opencode_config_hides_every_secret_shape() {
    let config = json!({
        "provider": { "acme": { "options": { "apiKey": CANARY, "baseURL": "https://api.acme.dev/v1" } } },
        "mcp": {
            "github": {
                "type": "local",
                "command": ["npx", "-y", "@modelcontextprotocol/server-github"],
                "environment": { "GITHUB_ORG": CANARY, "RETRIES": 3 },
            },
            "linear": {
                "type": "remote",
                "url": "https://mcp.linear.app/sse",
                "headers": { "Authorization": CANARY, "X-Org": CANARY },
                "oauth": { "clientId": "desktop-client", "clientSecret": CANARY, "scope": "repo read:org" },
            },
        },
    });

    let redacted = Redactor.redact(&config);
    let hidden = ConfigValue::Secret(SecretKind::Value);

    for path in [
        &["provider", "acme", "options", "apiKey"][..],
        &["mcp", "github", "environment", "GITHUB_ORG"],
        &["mcp", "github", "environment", "RETRIES"],
        &["mcp", "linear", "headers", "Authorization"],
        &["mcp", "linear", "headers", "X-Org"],
        &["mcp", "linear", "oauth", "clientSecret"],
    ] {
        assert_eq!(field(&redacted, path), &hidden, "{path:?} must be hidden");
    }

    let exposed = serde_json::to_string(&redacted).unwrap();
    assert!(!exposed.contains(CANARY), "canary leaked: {exposed}");

    // Positive control: non-secret configuration is still visible.
    for visible in [
        "https://api.acme.dev/v1",
        "@modelcontextprotocol/server-github",
        "desktop-client",
        "repo read:org",
    ] {
        assert!(exposed.contains(visible), "expected {visible} to stay visible");
    }
}

#[test]
fn redacting_an_opencode_value_at_its_key_path_hides_secrets() {
    let cases = [
        (&["mcp", "github", "environment"][..], json!({ "GITHUB_ORG": CANARY })),
        (&["mcp", "linear", "headers"], json!({ "X-Org": CANARY })),
        (&["mcp", "linear", "oauth"], json!({ "clientSecret": CANARY })),
        (&["provider", "acme", "options"], json!({ "apiKey": CANARY })),
    ];

    for (path, value) in cases {
        let redacted = Redactor.redact_at(&value, &mut RedactionContext::at(path.iter().copied()));
        let exposed = serde_json::to_string(&redacted).unwrap();

        assert!(!exposed.contains(CANARY), "canary leaked at {path:?}: {exposed}");
    }
}
