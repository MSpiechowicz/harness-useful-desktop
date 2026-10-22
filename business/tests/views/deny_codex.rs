//! The deny-list for Codex's private files: its login store, prompt history, session rollouts and
//! titles, shell snapshots, memories, logs, databases, caches and app state are never read, in
//! `~/.codex` or in a home `CODEX_HOME` names, while its settings, instructions, skills, rules,
//! plugins and marketplace clones stay readable and same-named folders elsewhere are not caught.

use std::path::{Path, PathBuf};

use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::security::{DenyReason, SafeReader, is_denied};
use pretty_assertions::assert_eq;

use crate::common::{codex_home, files_below};

#[test]
fn the_codex_login_store_is_denied_in_any_home() {
    for path in [
        "/h/.codex/auth.json",
        "C:/Users/me/.codex/Auth.json",
        "/h/codex-work/auth.json",
    ] {
        assert_eq!(
            is_denied(Path::new(path)),
            Some(DenyReason::FileName("auth.json".into())),
            "{path} must be denied"
        );
    }
}

#[test]
fn codex_private_files_and_folders_are_denied() {
    let denied = [
        "/h/.codex/history.jsonl",
        "/h/.codex/session_index.jsonl",
        "/h/.codex/transcription-history.jsonl",
        "/h/.codex/installation_id",
        "/h/.codex/.codex-global-state.json",
        "/h/.codex/.codex-global-state.json.bak",
        "/h/.codex/config.toml.backup",
        "/h/.codex/memories_1.sqlite",
        "/h/.codex/memories_1.sqlite-wal",
        "/h/.codex/state_5.sqlite",
        "/h/.codex/sessions/2026/10/22/rollout-2026-10-22T10-00-00-0b5e6c1a.jsonl",
        "/h/.codex/archived_sessions/rollout-old.jsonl",
        "/h/.codex/shell_snapshots/snapshot.sh",
        "/h/.codex/mcp-oauth-locks/docs.lock",
        "/h/.codex/computer-use/state.json",
        "/h/.codex/dictation-history/1.txt",
        "/h/.codex/cache/remote_plugin_catalog.json",
        "/h/.codex/log/codex-tui.txt",
        "/h/.codex/sqlite/x.json",
        "C:/Users/me/.Codex/History.jsonl",
    ];

    for path in denied {
        assert!(is_denied(Path::new(path)).is_some(), "{path} must be denied");
    }

    assert_eq!(
        is_denied(Path::new("/h/.codex/sessions/2026/x.jsonl")),
        Some(DenyReason::Directory("sessions".into()))
    );
}

#[test]
fn codex_settings_extensions_and_same_named_folders_stay_readable() {
    let allowed = [
        "/h/.codex/config.toml",
        "/h/.codex/AGENTS.md",
        "/h/.codex/skills/demo/SKILL.md",
        "/h/.codex/skills/.system/imagegen/SKILL.md",
        "/h/.codex/rules/default.rules",
        "/h/.codex/plugins/cache/market/demo/1.0.0/.codex-plugin/plugin.json",
        "/h/.codex/.tmp/marketplaces/market/.agents/plugins/marketplace.json",
        "/h/proj/.codex/config.toml",
        "/h/proj/AGENTS.md",
        // Only names directly in a Codex home count.
        "/h/proj/sessions/notes.md",
        "/h/proj/history.jsonl",
        "/h/proj/cache/data.json",
    ];

    for path in allowed {
        assert_eq!(is_denied(Path::new(path)), None, "{path} must be allowed");
    }
}

#[test]
fn a_renamed_codex_home_refuses_its_private_files() {
    let home = codex_home();
    let work = home.join("codex-work");
    let _env = Env::for_tests(&home).with_var("CODEX_HOME", work.to_string_lossy());

    let readable = [work.join("config.toml"), work.join("skills/notes/SKILL.md")];
    let private: Vec<PathBuf> = files_below(&work)
        .into_iter()
        .filter(|path| !readable.contains(path))
        .collect();

    assert!(private.len() >= 5, "the fixture keeps its private files");

    for path in private {
        let read = SafeReader::read_to_string(&path, SafeReader::DEFAULT_MAX_BYTES);

        assert!(
            matches!(read, Err(BusinessError::Forbidden(_))),
            "{} must be refused",
            path.display()
        );
    }

    for path in readable {
        assert!(SafeReader::read_to_string(&path, SafeReader::DEFAULT_MAX_BYTES).is_ok());
    }
}

#[test]
fn the_fixture_private_files_are_refused() {
    let codex = codex_home().join(".codex");
    let private = [
        "auth.json",
        "history.jsonl",
        "session_index.jsonl",
        "transcription-history.jsonl",
        "installation_id",
        ".codex-global-state.json",
        "config.toml.backup",
        "memories_1.sqlite-wal",
        "sessions/2026/10/22/rollout-2026-10-22T10-00-00-0b5e6c1a.jsonl",
        "archived_sessions/rollout-old.jsonl",
        "shell_snapshots/snapshot.sh",
        "mcp-oauth-locks/docs.lock",
        "computer-use/state.json",
        "dictation-history/1.txt",
        "cache/remote_plugin_catalog.json",
        "log/codex-tui.txt",
    ];

    for name in private {
        let path = codex.join(name);
        assert!(path.is_file(), "{} is in the fixture", path.display());

        let read = SafeReader::read_to_string(&path, SafeReader::DEFAULT_MAX_BYTES);
        assert!(
            matches!(read, Err(BusinessError::Forbidden(_))),
            "{} must be refused",
            path.display()
        );
    }
}
