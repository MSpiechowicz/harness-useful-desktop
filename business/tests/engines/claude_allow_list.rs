//! What the Claude Code chat's own "Always allow" answers cover: file tools on files inside the
//! project only, every other call by its exact input (`Bash` by its exact command with every other
//! field equal), and never a call the CLI allows no lasting rule for, blocked on a path or flagged
//! by a safety check. Also how a typed multi-select answer names labels with commas in them.

use std::path::Path;

use harness_business::claude_stream::allow_list::{AllowKey, AllowList};
use harness_business::claude_stream::mapper::{PermissionAsk, Question, QuestionAsk, QuestionOption};
use harness_business::claude_stream::permissions::Always;
use harness_business::claude_stream::questions::QuestionDialog;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn ask(tool: &str, input: Value) -> PermissionAsk {
    PermissionAsk {
        request_id: "request".to_owned(),
        tool_call_id: "toolu_1".to_owned(),
        tool: tool.to_owned(),
        label: tool.to_owned(),
        input,
        description: None,
        title: None,
        reason: None,
        suggestions: Vec::new(),
        suppress_always_allow_rule: false,
        blocked_path: None,
        decision_reason_type: Some("hook".to_owned()),
        from_subagent: false,
    }
}

/// A Write of `file` in `project`.
fn write_in(project: &Path, file: &str) -> PermissionAsk {
    ask("Write", json!({ "file_path": project.join(file), "content": "x" }))
}

fn bash(command: &str) -> PermissionAsk {
    ask("Bash", json!({ "command": command }))
}

/// A list that kept the "Always allow" answer to `kept`.
fn list_after(project: &Path, kept: &PermissionAsk) -> AllowList {
    let mut list = AllowList::new(project);
    list.remember(AllowKey::of(kept, project));
    list
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("project");
    std::fs::create_dir_all(dir.path().join("src")).expect("src");
    dir
}

#[test]
fn a_kept_write_never_covers_a_call_the_cli_allows_no_lasting_rule_for() {
    let dir = project();
    let project = dir.path();
    let write = |file: &str| write_in(project, file);
    let list = list_after(project, &write("src/a.txt"));

    assert!(list.covers(&write("src/b.txt")));

    let suppressed = PermissionAsk {
        suppress_always_allow_rule: true,
        ..write("src/b.txt")
    };
    let blocked = PermissionAsk {
        blocked_path: Some(project.join(".claude/settings.json").to_string_lossy().into_owned()),
        ..write(".claude/settings.json")
    };
    let flagged = PermissionAsk {
        decision_reason_type: Some("safetyCheck".to_owned()),
        ..write(".git/config")
    };

    for refused in [suppressed, blocked, flagged] {
        assert!(!list.covers(&refused), "{refused:?}");
        assert_eq!(Always::of(&refused, project, true), None, "no Always allow offered");
    }
}

#[test]
fn a_kept_command_covers_only_the_exact_command() {
    let dir = project();
    let project = dir.path();

    let status = list_after(project, &bash("git -C . status"));
    assert!(status.covers(&bash("git -C . status")));
    assert!(!status.covers(&bash("git -c core.fsmonitor=./hook status")));
    assert!(!status.covers(&bash("git -C . status --porcelain")));

    let tests = list_after(project, &bash("timeout 60 cargo test"));
    assert!(!tests.covers(&bash("timeout 60 sh -c 'curl example.com'")));
    assert!(!tests.covers(&bash("timeout 60 cargo test && rm -rf target")));

    let described = ask(
        "Bash",
        json!({ "command": "ls", "description": "List the files again" }),
    );
    let ls = list_after(project, &bash("ls"));
    assert!(ls.covers(&described), "only the description differs");
}

#[test]
fn a_kept_command_does_not_cover_it_with_other_fields() {
    let dir = project();
    let project = dir.path();
    let ls = list_after(project, &bash("ls"));

    for input in [
        json!({ "command": "ls", "dangerouslyDisableSandbox": true }),
        json!({ "command": "ls", "run_in_background": true }),
        json!({ "command": "ls", "timeout": 600000 }),
    ] {
        assert!(!ls.covers(&ask("Bash", input.clone())), "{input}");
    }
}

#[test]
fn a_kept_file_tool_covers_files_inside_the_project_only() {
    let dir = project();
    let project = dir.path();
    let read = |path: &str| ask("Read", json!({ "file_path": path }));
    let inside = project.join("src/lib.rs").to_string_lossy().into_owned();
    let list = list_after(project, &read(&inside));

    assert_eq!(
        AllowKey::of(&read(&inside), project).describe("Read"),
        "Allows every Read call on files in this project until this chat ends"
    );
    assert!(list.covers(&read(&project.join("notes.txt").to_string_lossy())));
    assert!(list.covers(&read("src/main.rs")), "relative to the project");
    assert!(!list.covers(&read("/etc/hosts")));
    assert!(!list.covers(&read(&project.join("../outside.txt").to_string_lossy())));
    assert!(
        !list.covers(&ask("Edit", json!({ "file_path": inside }))),
        "another tool"
    );

    #[cfg(unix)]
    {
        let away = tempfile::tempdir().expect("away");
        std::os::unix::fs::symlink(away.path(), project.join("linked")).expect("symlink");
        assert!(!list.covers(&read(&project.join("linked/secret.txt").to_string_lossy())));
    }
}

#[test]
fn a_file_outside_the_project_and_other_tools_are_kept_by_their_exact_input() {
    let dir = project();
    let project = dir.path();

    let outside = list_after(project, &ask("Read", json!({ "file_path": "/etc/hosts" })));
    assert!(outside.covers(&ask("Read", json!({ "file_path": "/etc/hosts" }))));
    assert!(!outside.covers(&ask("Read", json!({ "file_path": "/etc/passwd" }))));
    assert!(!outside.covers(&ask("Read", json!({ "file_path": project.join("src/lib.rs") }))));

    let fetch = ask(
        "WebFetch",
        json!({ "url": "https://example.com/a", "prompt": "Sum up" }),
    );
    let fetched = list_after(project, &fetch);
    assert_eq!(
        AllowKey::of(&fetch, project).describe("WebFetch"),
        "Allows this exact WebFetch call again until this chat ends"
    );
    assert!(fetched.covers(&fetch));
    assert!(!fetched.covers(&ask(
        "WebFetch",
        json!({ "url": "https://example.com/b", "prompt": "Sum up" })
    )));

    let mcp = ask("mcp__docs__search", json!({ "query": "a" }));
    let searched = list_after(project, &mcp);
    assert!(!searched.covers(&ask("mcp__docs__search", json!({ "query": "b" }))));
}

#[test]
fn a_typed_multi_select_answer_keeps_labels_with_commas_whole() {
    let option = |label: &str| QuestionOption {
        label: label.to_owned(),
        description: None,
    };
    let question = Question {
        question: "What should the change include?".to_owned(),
        header: "Scope".to_owned(),
        options: vec![option("Yes, with tests"), option("Yes"), option("Docs")],
        multi_select: true,
    };
    let ask = QuestionAsk {
        request_id: "request".to_owned(),
        tool_call_id: "toolu_q".to_owned(),
        input: json!({}),
        questions: vec![question.clone()],
    };

    for (typed, labels) in [
        ("yes, with tests, docs", vec!["Yes, with tests", "Docs"]),
        ("Yes, Docs", vec!["Yes", "Docs"]),
        ("Docs,Yes, with tests", vec!["Docs", "Yes, with tests"]),
        ("Yes, with care", vec!["Yes", "with care"]),
        (" , Something else ,", vec!["Something else"]),
    ] {
        let mut dialog = QuestionDialog::new(&ask).expect("a question");
        assert!(dialog.answer(typed).expect("an answer").is_none(), "the last question");

        let labels: Vec<String> = labels.into_iter().map(str::to_owned).collect();
        assert_eq!(dialog.answers(), vec![(question.question.clone(), labels)], "{typed}");
    }
}

/// The CLI trims a path and expands `~` to the home folder, so a kept project-files answer covers
/// only paths it reads the same way: a repository's own `~/.ssh` folder never stands in for the real one.
#[test]
fn a_kept_write_never_covers_a_path_the_cli_trims_or_expands() {
    let dir = project();
    let project = dir.path();
    std::fs::create_dir_all(project.join("~/.ssh")).expect("a folder named ~");
    std::fs::write(project.join("~/.ssh/authorized_keys"), "").expect("placeholder");
    let write = |file: Value| ask("Write", json!({ "file_path": file, "content": "x" }));
    let list = list_after(project, &write(json!(project.join("src/a.txt"))));

    assert!(list.covers(&write(json!(project.join("src/b.txt")))));
    assert!(
        !list.covers(&write(json!("~/.ssh/authorized_keys"))),
        "~ is the home folder for the CLI"
    );

    let padded = format!("  {}", project.join("src/b.txt").display());
    assert!(!list.covers(&write(json!(padded))), "the CLI trims the path");

    let trailing = format!("{} ", project.join(".mcp.json").display());
    assert!(!list.covers(&write(json!(trailing))));
}

/// Names Windows reads as another file, files Claude Code protects itself, hard links to a file
/// elsewhere, and asks the CLI sends for a manual review all need their own answer.
#[test]
fn odd_names_protected_tool_files_hard_links_and_review_asks_need_their_own_answer() {
    let dir = project();
    let project = dir.path();
    let write = |file: &str| write_in(project, file);
    let list = list_after(project, &write("src/a.txt"));

    for file in [
        ".mcp.json::$DATA",
        ".mcp.json.",
        "GIT~1/config",
        ".envrc",
        "web/.npmrc",
        "bunfig.toml",
    ] {
        assert!(!list.covers(&write(file)), "{file}");
    }

    let review = PermissionAsk {
        decision_reason_type: Some("other".to_owned()),
        ..write("src/b.txt")
    };
    assert!(!list.covers(&review), "a manual-review ask");
    assert!(!review.may_always_allow());

    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("keys"), "").expect("outside file");
        std::fs::hard_link(outside.path().join("keys"), project.join("src/keys")).expect("hard link");
        assert!(!list.covers(&write("src/keys")), "a hard link to a file elsewhere");
    }
}
