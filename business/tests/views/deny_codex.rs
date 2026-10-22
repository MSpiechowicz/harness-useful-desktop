//! The deny-list for Codex's login store: `auth.json` is never read, in `~/.codex` or in a home
//! `CODEX_HOME` names, while its settings, instructions, skills, rules and session files stay
//! readable.

use std::path::Path;

use harness_business::BusinessError;
use harness_business::security::{DenyReason, SafeReader, is_denied};
use pretty_assertions::assert_eq;

use crate::common::codex_home;

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
fn codex_settings_extensions_and_sessions_stay_readable() {
    let allowed = [
        "/h/.codex/config.toml",
        "/h/.codex/AGENTS.md",
        "/h/.codex/skills/demo/SKILL.md",
        "/h/.codex/rules/default.rules",
        "/h/.codex/sessions/2026/10/22/rollout-2026-10-22T10-00-00-0b5e6c1a.jsonl",
        "/h/proj/.codex/config.toml",
        "/h/proj/AGENTS.md",
    ];

    for path in allowed {
        assert_eq!(is_denied(Path::new(path)), None, "{path} must be allowed");
    }
}

#[test]
fn the_fixture_login_stores_are_refused() {
    let home = codex_home();

    for path in [home.join(".codex/auth.json"), home.join("codex-work/auth.json")] {
        let read = SafeReader::read_to_string(&path, SafeReader::DEFAULT_MAX_BYTES);

        assert!(
            matches!(read, Err(BusinessError::Forbidden(_))),
            "{} must be refused",
            path.display()
        );
    }
}
