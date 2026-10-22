//! The deny-list for Claude Code's private files: its login store, prompt history, caches,
//! backups, telemetry and full tool outputs are never read, while its settings, skills, agents,
//! commands, plugins and session files stay readable and same-named project folders are not caught.
//! The private names also apply in a config directory `CLAUDE_CONFIG_DIR` gives another name.

use std::path::Path;

use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::security::{DenyReason, SafeReader, is_denied};
use pretty_assertions::assert_eq;

#[test]
fn deny_list_covers_claude_private_files_and_folders() {
    let denied = [
        "/h/.claude/.credentials.json",
        "C:/Users/me/.claude/.Credentials.json",
        "/h/.claude/history.jsonl",
        "/h/.claude/remote-settings.json",
        "/h/.claude/policy-limits.json",
        "/h/.claude/policy-limits.stamp.json",
        "/h/.claude/stats-cache.json",
        "/h/.claude/mcp-needs-auth-cache.json",
        "/h/.claude.json.backup",
        "/h/.claude.json.backup.1760000000000",
        "/h/.claude/backups/.claude.json.backup.1760000000000",
        "/h/.claude/dump-1.heapsnapshot",
        "/h/.claude/paste-cache/abc.txt",
        "/h/.claude/shell-snapshots/snapshot-bash-1.sh",
        "/h/.claude/file-history/0b5e6c1a/x.ts@v1",
        "/h/.claude/session-env/0b5e6c1a/env",
        "/h/.claude/statsig/statsig.cached.evaluations.json",
        "/h/.claude/debug/0b5e6c1a.txt",
        "/h/.claude/uploads/pasted.txt",
        "/h/.claude/image-cache/img.png",
        "/h/.claude/chrome/state.json",
        "/h/.claude/ide/41234.lock",
        "/h/.claude/feedback/report.json",
        "/h/.claude/feedback-bundles/report.json",
        "/h/.claude/plugins/data/demo-plugin/state.json",
        "/h/.claude/projects/-h-demo/0b5e6c1a/tool-results/toolu_1.txt",
        // CLAUDE_CONFIG_DIR may give the directory any name.
        "/h/claude-work/projects/-h-demo/0b5e6c1a/tool-results/toolu_1.txt",
        "/h/claude-work/.credentials.json",
        "/h/claude-work/paste-cache/abc.txt",
    ];

    for path in denied {
        assert!(is_denied(Path::new(path)).is_some(), "{path} must be denied");
    }

    assert_eq!(
        is_denied(Path::new("/h/.claude/history.jsonl")),
        Some(DenyReason::FileName("history.jsonl".into()))
    );
    assert_eq!(
        is_denied(Path::new("/h/.claude/plugins/data/x/state.json")),
        Some(DenyReason::Directory("data".into()))
    );
}

#[test]
fn claude_settings_extensions_and_sessions_stay_readable() {
    let allowed = [
        "/h/.claude.json",
        "/h/.claude/settings.json",
        "/h/.claude/settings.local.json",
        "/h/.claude/CLAUDE.md",
        "/h/.claude/skills/demo/SKILL.md",
        "/h/.claude/agents/reviewer.md",
        "/h/.claude/commands/test.md",
        "/h/.claude/plugins/installed_plugins.json",
        "/h/.claude/plugins/known_marketplaces.json",
        "/h/.claude/plugins/marketplaces/demo/.claude-plugin/marketplace.json",
        "/h/.claude/plugins/cache/demo/plugin/1.0.0/skills/x/SKILL.md",
        "/h/.claude/projects/-h-demo/0b5e6c1a-0000-4000-8000-000000000001.jsonl",
        "/h/proj/.claude/settings.json",
        "/h/proj/.mcp.json",
    ];

    for path in allowed {
        assert_eq!(is_denied(Path::new(path)), None, "{path} must be allowed");
    }
}

#[test]
fn generic_folder_and_file_names_outside_the_claude_dir_are_not_caught() {
    let allowed = [
        "/h/proj/history.jsonl",
        "/h/proj/remote-settings.json",
        "/h/proj/stats-cache.json",
        "/h/proj/backups/notes.md",
        "/h/proj/debug/main.rs",
        "/h/proj/src/debug/mod.rs",
        "/h/proj/uploads/readme.md",
        "/h/proj/chrome/manifest.json",
        "/h/proj/ide/settings.json",
        "/h/proj/statsig/client.ts",
        "/h/proj/image-cache/index.ts",
        "/h/proj/feedback/form.svelte",
        "/h/proj/plugins/data/schema.json",
        "/h/proj/tool-results/out.txt",
        "/h/.omp/agent/plugins/data/x.json",
    ];

    for path in allowed {
        assert_eq!(is_denied(Path::new(path)), None, "{path} must be allowed");
    }
}

/// A config directory `CLAUDE_CONFIG_DIR` gives another name: an environment naming it registers
/// it, so the private names directly inside it are refused like in `~/.claude`.
#[test]
fn claude_private_names_are_denied_under_a_renamed_config_directory() {
    let root = if cfg!(windows) {
        "C:/h/claude-renamed"
    } else {
        "/h/claude-renamed"
    };
    let _env = Env::for_tests("/h").with_var("CLAUDE_CONFIG_DIR", root);

    for private in [
        "history.jsonl",
        "remote-settings.json",
        "stats-cache.json",
        "backups/notes.json",
        "statsig/evaluations.json",
        "debug/0b5e6c1a.txt",
        "uploads/pasted.txt",
        "image-cache/img.png",
        "chrome/state.json",
        "ide/41234.lock",
        "feedback/report.json",
        "feedback-bundles/report.json",
        "plugins/data/demo-plugin/state.json",
    ] {
        let path = format!("{root}/{private}");
        assert!(is_denied(Path::new(&path)).is_some(), "{path} must be denied");
    }

    // Names are compared without case, like the `.claude` rules.
    let shouted = if cfg!(windows) {
        "C:/H/Claude-Renamed/History.jsonl"
    } else {
        "/H/Claude-Renamed/History.jsonl"
    };
    assert!(is_denied(Path::new(shouted)).is_some());

    for readable in [
        "settings.json",
        ".claude.json",
        "CLAUDE.md",
        "skills/demo/SKILL.md",
        "plugins/installed_plugins.json",
        "projects/-h-demo/0b5e6c1a-0000-4000-8000-000000000001.jsonl",
        // Only the config directory's own children are private.
        "skills/demo/debug/notes.md",
        "projects/-h-demo/history.jsonl",
    ] {
        let path = format!("{root}/{readable}");
        assert_eq!(is_denied(Path::new(&path)), None, "{path} must be allowed");
    }

    for elsewhere in [
        "/h/claude-renamed-not/history.jsonl",
        "/h/proj/history.jsonl",
        "/h/proj/debug/main.rs",
    ] {
        assert_eq!(is_denied(Path::new(elsewhere)), None, "{elsewhere} must be allowed");
    }
}

#[test]
fn a_relative_config_directory_is_not_registered() {
    let _env = Env::for_tests("/h").with_var("CLAUDE_CONFIG_DIR", "relative-claude");

    assert_eq!(is_denied(Path::new("relative-claude/history.jsonl")), None);
}

/// The deny check sees the canonical path too, so a renamed config directory reached through a
/// symlink keeps its private files refused on either path.
#[cfg(unix)]
#[test]
fn a_symlinked_config_directory_is_covered_on_its_real_path() {
    let temp = tempfile::tempdir().unwrap();
    let real = temp.path().join("real-claude");
    let link = temp.path().join("linked-claude");
    std::fs::create_dir_all(real.join("debug")).unwrap();
    std::fs::write(real.join("history.jsonl"), "{}").unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let _env = Env::for_tests(temp.path()).with_var("CLAUDE_CONFIG_DIR", link.to_string_lossy());

    assert!(is_denied(&link.join("history.jsonl")).is_some());
    assert!(is_denied(&dunce::canonicalize(&real).unwrap().join("history.jsonl")).is_some());
    assert!(matches!(
        SafeReader::read_to_string(&link.join("history.jsonl"), 1024),
        Err(BusinessError::Forbidden(_))
    ));
}
