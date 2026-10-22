//! "Always allow" on the permission asks Claude Code 2.1.304 really sends for protected files and
//! for a file outside the project (`fixtures/claude/stream/protected_default.jsonl` and
//! `protected_askall.jsonl`, the project moved to a temporary folder): a safety check never offers
//! it, a protected file or a `workingDir` ask is never covered by "files in this project", an
//! echoed `addDirectories` names its folder, and a dangling symlink in the project counts as
//! outside it.

mod claude_recorded;

use std::path::Path;

use claude_recorded::{Dir, frames, moved};
use harness_business::claude_stream::ClaudeMapper;
use harness_business::claude_stream::allow_list::{AllowKey, AllowList};
use harness_business::claude_stream::mapper::{Ask, Hint, Mapped, PermissionAsk};
use harness_business::claude_stream::permissions::{Always, permission_dialog};
use harness_business::model::{AgentEvent, UiRequestKind};
use pretty_assertions::assert_eq;
use serde_json::json;

/// The detail of a ProjectFiles key for `Write`.
#[cfg(unix)]
const PROJECT_FILES: &str = "Allows every Write call on files in this project until this chat ends";
/// The detail of an exact key for `Write`.
const EXACT: &str = "Allows this exact Write call again until this chat ends";

/// The permission asks of recording `file`, its project moved to `project`.
fn recorded_asks(file: &str, project: &Path) -> Vec<PermissionAsk> {
    let mut mapper = ClaudeMapper::new(project);

    frames(file, Dir::Out)
        .into_iter()
        .filter(|frame| frame["request"]["subtype"] == "can_use_tool")
        .flat_map(|frame| mapper.on_line(&moved(&frame, project)))
        .filter_map(|mapped| match mapped {
            Mapped::Hint(Hint::Ask(Ask::Permission(ask))) => Some(*ask),
            _ => None,
        })
        .collect()
}

/// The ask of `asks` on a file whose path ends with `suffix`.
fn ask_on<'a>(asks: &'a [PermissionAsk], suffix: &str) -> &'a PermissionAsk {
    asks.iter()
        .find(|ask| {
            ask.input["file_path"]
                .as_str()
                .is_some_and(|path| path.ends_with(suffix))
        })
        .unwrap_or_else(|| panic!("an ask on {suffix}"))
}

/// The same ask on `file` (relative to `project`).
fn on_file(ask: &PermissionAsk, project: &Path, file: &str) -> PermissionAsk {
    PermissionAsk {
        input: json!({ "file_path": project.join(file), "content": "x" }),
        ..ask.clone()
    }
}

/// The dialog details of `ask`: `[Allow once, Always allow?, Reject]`.
fn details(ask: &PermissionAsk, project: &Path, ask_all: bool) -> Vec<Option<String>> {
    let always = Always::of(ask, project, ask_all);
    let AgentEvent::UiRequest { request } = permission_dialog(ask, project, always.as_ref()) else {
        panic!("a dialog");
    };
    let UiRequestKind::Select { details, .. } = request.kind else {
        panic!("a select");
    };

    details
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("project");
    std::fs::create_dir_all(dir.path().join("src")).expect("src");
    dir
}

#[test]
fn recorded_safety_checks_on_protected_files_never_offer_always_allow() {
    let dir = project();
    let project = dir.path();

    for (file, ask_all) in [("protected_default.jsonl", false), ("protected_askall.jsonl", true)] {
        let asks = recorded_asks(file, project);

        for suffix in [".claude/settings.json", ".git/hooks/pre-commit"] {
            let ask = ask_on(&asks, suffix);

            assert_eq!(
                ask.decision_reason_type.as_deref(),
                Some("safetyCheck"),
                "{file} {suffix}"
            );
            assert_eq!(Always::of(ask, project, ask_all), None, "{file} {suffix}");
            assert_eq!(details(ask, project, ask_all).len(), 2, "Allow once and Reject only");
        }
    }
}

#[test]
fn protected_files_are_never_covered_as_files_in_this_project() {
    let dir = project();
    let project = dir.path();
    let asks = recorded_asks("protected_askall.jsonl", project);
    // The ask-all hook masks the CLI's reason: the same asks as `hook` asks.
    let hooked = PermissionAsk {
        decision_reason_type: Some("hook".to_owned()),
        ..ask_on(&asks, ".claude/settings.json").clone()
    };
    let list = {
        let mut list = AllowList::new(project);
        list.remember(AllowKey::of(&on_file(&hooked, project, "src/a.txt"), project));
        list
    };

    assert!(list.covers(&on_file(&hooked, project, "src/b.txt")));

    for protected in [
        ".claude/settings.json",
        ".git/hooks/pre-commit",
        ".Git/config",
        "sub/.CLAUDE/agents/x.md",
        ".vscode/settings.json",
        ".idea/workspace.xml",
        ".mcp.json",
        "sub/.gitmodules",
        ".gitconfig",
        ".bashrc",
        ".zshrc",
        ".profile",
        ".Bash_Profile",
        "conf/config.fish",
    ] {
        let ask = on_file(&hooked, project, protected);

        assert!(!list.covers(&ask), "{protected}");
        assert_eq!(AllowKey::of(&ask, project).describe("Write"), EXACT, "{protected}");
    }
}

#[test]
fn an_ask_outside_the_working_directories_is_never_covered_as_a_project_file() {
    let dir = project();
    let project = dir.path();
    let asks = recorded_asks("protected_default.jsonl", project);
    let outside = ask_on(&asks, "/tmp/cc-rec-outside.txt");

    assert_eq!(outside.decision_reason_type.as_deref(), Some("workingDir"));

    // Even when the app finds the file inside the project, the CLI's reason wins.
    let inside = on_file(outside, project, "src/a.txt");
    assert_eq!(AllowKey::of(&inside, project).describe("Write"), EXACT);

    let hooked = PermissionAsk {
        decision_reason_type: Some("hook".to_owned()),
        ..inside.clone()
    };
    let mut list = AllowList::new(project);
    list.remember(AllowKey::of(&hooked, project));

    assert!(list.covers(&on_file(&hooked, project, "src/b.txt")));
    assert!(!list.covers(&on_file(&inside, project, "src/b.txt")));
}

#[test]
fn always_allow_on_an_outside_write_names_the_folder_it_allows() {
    let dir = project();
    let project = dir.path();
    let asks = recorded_asks("protected_default.jsonl", project);
    let outside = ask_on(&asks, "/tmp/cc-rec-outside.txt");

    assert_eq!(
        Always::of(outside, project, false),
        Some(Always::Rules(vec![json!({
            "type": "addDirectories",
            "directories": ["/tmp"],
            "destination": "session",
        })]))
    );
    assert_eq!(
        details(outside, project, false)[1].as_deref(),
        Some("Also allows access to /tmp until this chat ends")
    );
}

#[test]
fn an_outside_write_in_ask_all_mode_is_kept_by_its_exact_input() {
    let dir = project();
    let project = dir.path();
    let asks = recorded_asks("protected_askall.jsonl", project);
    let outside = ask_on(&asks, "/tmp/cc-rec-outside.txt");

    assert_eq!(outside.decision_reason_type.as_deref(), Some("hook"));
    assert_eq!(
        details(outside, project, true)[1].as_deref(),
        Some(EXACT),
        "the masked reason does not widen the key"
    );

    let mut list = AllowList::new(project);
    list.remember(AllowKey::of(outside, project));

    let other = PermissionAsk {
        input: json!({ "file_path": "/tmp/cc-rec-other.txt", "content": "hi" }),
        ..outside.clone()
    };
    assert!(list.covers(outside));
    assert!(!list.covers(&other));
}

#[cfg(unix)]
#[test]
fn a_dangling_symlink_in_the_project_counts_as_outside_it() {
    use harness_business::pi_family::touched::touched_paths;

    let dir = project();
    let project = dir.path();
    let away = tempfile::tempdir().expect("away");
    std::os::unix::fs::symlink(away.path().join("missing.txt"), project.join("src/link.txt")).expect("symlink");
    std::os::unix::fs::symlink(project.join("src/gone.txt"), project.join("src/inner.txt")).expect("symlink");

    let asks = recorded_asks("protected_askall.jsonl", project);
    let hooked = PermissionAsk {
        decision_reason_type: Some("hook".to_owned()),
        ..ask_on(&asks, ".claude/settings.json").clone()
    };
    let kept = on_file(&hooked, project, "src/a.txt");
    let mut list = AllowList::new(project);
    list.remember(AllowKey::of(&kept, project));

    assert_eq!(AllowKey::of(&kept, project).describe("Write"), PROJECT_FILES);

    for link in ["src/link.txt", "src/inner.txt"] {
        let path = project.join(link);
        let through = on_file(&hooked, project, link);

        assert!(!list.covers(&through), "{link}: the dialog shows");
        assert!(
            touched_paths(project, "write", &json!({ "file_path": path })).is_empty(),
            "{link}: not a touched project file"
        );
    }
}
