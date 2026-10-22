//! The deny-list for Claude Code's private files: its login store, prompt history, caches,
//! backups, telemetry and full tool outputs are never read, while its settings, skills, agents,
//! commands, plugins and session files stay readable and same-named project folders are not caught.

use std::path::Path;

use harness_business::security::{DenyReason, is_denied};
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
