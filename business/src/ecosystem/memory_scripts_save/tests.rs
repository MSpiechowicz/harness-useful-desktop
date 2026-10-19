use pretty_assertions::assert_eq;

use super::*;
use crate::model::{ScriptApproval, ScriptLanguage};

fn project() -> Option<&'static Path> {
    Some(Path::new("/work/app"))
}

fn draft() -> ScriptDraft {
    ScriptDraft {
        name: " open-prs ".to_owned(),
        description: " Lists the open pull requests ".to_owned(),
        language: ScriptLanguage::Bash,
        body: "gh pr list\n".to_owned(),
        timeout_secs: 60,
        scope: ScriptScope::Repo,
    }
}

fn output(structured: Option<Value>, text: &str, is_error: bool) -> ToolOutput {
    ToolOutput {
        structured,
        text: Some(text.to_owned()),
        is_error,
    }
}

fn summary(id: i64) -> Value {
    json!({
        "id": id, "name": "open-prs", "description": "Lists the open pull requests", "language": "bash",
        "scope": "repo", "repo": "github.com/o/r", "sha256": "ab".repeat(32), "timeoutSecs": 60,
        "place": "local", "author": { "source": "harness-desktop", "root": null },
        "createdAt": "2026-10-19T08:00:00Z", "updatedAt": "2026-10-19T08:00:00Z", "runCount": 0,
        "lastRunAt": null, "lastExitCode": null, "trusted": true, "approval": "local",
    })
}

#[test]
fn the_draft_goes_out_trimmed() {
    let fields = checked(&draft(), project()).unwrap();

    assert_eq!(
        Value::Object(fields),
        json!({
            "scope": "repo", "name": "open-prs", "description": "Lists the open pull requests",
            "language": "bash", "body": "gh pr list\n", "timeout_secs": 60,
        })
    );
}

#[test]
fn a_draft_over_a_limit_is_refused() {
    let refused = |change: fn(&mut ScriptDraft)| {
        let mut draft = draft();
        change(&mut draft);
        matches!(checked(&draft, project()), Err(BusinessError::InvalidInput(_)))
    };

    assert!(refused(|draft| draft.name = "Open".to_owned()));
    assert!(refused(|draft| draft.name = "-open".to_owned()));
    assert!(refused(|draft| draft.name = "a".repeat(SCRIPT_NAME_MAX + 1)));
    assert!(refused(|draft| draft.description = " ".to_owned()));
    assert!(refused(
        |draft| draft.description = "d".repeat(SCRIPT_DESCRIPTION_MAX + 1)
    ));
    assert!(refused(|draft| draft.body = "\n".to_owned()));
    assert!(refused(|draft| draft.body = "a".repeat(SCRIPT_BODY_MAX + 1)));
    assert!(refused(|draft| draft.timeout_secs = 0));
    assert!(refused(|draft| draft.timeout_secs = SCRIPT_TIMEOUT_MAX + 1));

    let mut longest = draft();
    longest.name = "a".repeat(SCRIPT_NAME_MAX);
    longest.description = "d".repeat(SCRIPT_DESCRIPTION_MAX);
    longest.body = "a".repeat(SCRIPT_BODY_MAX);
    longest.timeout_secs = SCRIPT_TIMEOUT_MAX;
    assert!(checked(&longest, project()).is_ok(), "every limit is inclusive");

    assert!(
        matches!(checked(&draft(), None), Err(BusinessError::InvalidInput(_))),
        "a repository script without a project"
    );

    let mut global = draft();
    global.scope = ScriptScope::Global;
    assert!(checked(&global, None).is_ok());
}

#[test]
fn a_missing_name_lets_the_save_go_on_and_a_found_one_is_read_with_its_body() {
    let missing = output(None, "no script named open-prs here or in the global scope", true);
    assert_eq!(found(&missing).unwrap(), None);

    let mut script = summary(12);
    script["body"] = json!("gh pr list\n");
    let read = output(Some(json!({ "script": script })), "agent-written data", false);
    let detail = found(&read).unwrap().expect("found");
    assert_eq!((detail.summary.id, detail.body.as_str()), (12, "gh pr list\n"));

    let bare = output(Some(json!({ "script": summary(12) })), "agent-written data", false);
    assert!(
        matches!(found(&bare), Err(BusinessError::Protocol(_))),
        "a script without its body"
    );

    let broken = output(None, "memory database error: Busy", true);
    assert!(matches!(found(&broken), Err(BusinessError::Process(_))));
}

/// The script `draft()` stored as the server would answer it (waiting for approval).
fn stored(draft: &ScriptDraft) -> ScriptSummary {
    let mut script: ScriptSummary = parse(summary(12), "test").unwrap();
    script.sha256 = script_sha256(draft.language, &draft.body);
    script.approval = ScriptApproval::Needed;
    script.trusted = false;
    script
}

/// The script `draft()` replaces, with `body`.
fn replaced(body: &str) -> ScriptDetail {
    ScriptDetail {
        summary: parse(summary(12), "test").unwrap(),
        body: body.to_owned(),
    }
}

#[test]
fn only_code_the_user_changed_is_theirs() {
    let draft = draft();
    let agents = replaced("gh pr list --state all\n");

    assert!(
        users_code(&draft, &stored(&draft), Some(&agents), true),
        "an edit of the code"
    );
    assert!(users_code(&draft, &stored(&draft), None, false), "a new script");

    assert!(
        !users_code(&draft, &stored(&draft), Some(&replaced(&draft.body)), true),
        "the description, timeout or language edited over the same body"
    );

    let mut other_version = stored(&draft);
    other_version.sha256 = "cd".repeat(32);
    assert!(!users_code(&draft, &other_version, Some(&agents), true));

    assert!(
        !users_code(&draft, &stored(&draft), None, true),
        "an edit of a script that is not there"
    );
}

#[test]
fn line_endings_and_trailing_whitespace_are_not_a_code_change() {
    let edited = |body: &str| {
        let draft = ScriptDraft {
            body: body.to_owned(),
            ..draft()
        };
        users_code(
            &draft,
            &stored(&draft),
            Some(&replaced("gh pr list\n  --state all\n")),
            true,
        )
    };

    assert!(!edited("gh pr list\r\n  --state all\r\n"), "CRLF to LF");
    assert!(!edited("gh pr list\r  --state all"), "CR and no final newline");
    assert!(
        !edited("gh pr list \t\n  --state all  \n\n \n"),
        "trailing whitespace and blank lines"
    );
    assert!(edited("gh pr list\n--state all\n"), "leading whitespace is code");
    assert!(edited("gh pr list\n\n  --state all\n"), "a blank line inside is code");
}

#[test]
fn a_replace_over_untrusted_identical_code_keeps_it_waiting() {
    let draft = draft();
    let mut pending = replaced("gh pr list \r\n\n");
    pending.summary.trusted = false;
    pending.summary.approval = ScriptApproval::Needed;

    assert!(!users_code(&draft, &stored(&draft), Some(&pending), false));

    let other = ScriptDraft {
        body: "gh pr list --draft\n".to_owned(),
        ..draft.clone()
    };
    assert!(
        users_code(&other, &stored(&other), Some(&pending), false),
        "other code replaces it"
    );

    assert!(
        users_code(&draft, &stored(&draft), Some(&replaced(&draft.body)), false),
        "the same code as a trusted script"
    );
}

#[test]
fn an_edit_needs_the_version_it_opened() {
    let opened = replaced("gh pr list\n");
    let base = opened.summary.sha256.clone();

    assert!(unchanged(Some(&opened), &base).is_ok());
    assert!(matches!(
        unchanged(Some(&opened), &"cd".repeat(32)),
        Err(BusinessError::Conflict(ref message)) if message == CHANGED_MESSAGE
    ));
    assert!(
        matches!(unchanged(None, &base), Err(BusinessError::Conflict(_))),
        "deleted meanwhile"
    );
}

#[test]
fn a_stored_script_of_another_name_scope_or_repository_is_not_the_draft() {
    let draft = draft();
    let agents = replaced("gh pr list --state all\n");
    let differs = |change: fn(&mut ScriptSummary)| {
        let mut script = stored(&draft);
        change(&mut script);
        !users_code(&draft, &script, Some(&agents), true)
    };

    assert!(differs(|script| script.name = "close-prs".to_owned()));
    assert!(differs(|script| script.scope = ScriptScope::Global));
    assert!(differs(|script| script.repo = Some("github.com/evil/r".to_owned())));
    assert!(differs(|script| script.repo = None));

    let mut moved = replaced("gh pr list --state all\n");
    moved.summary.name = "close-prs".to_owned();
    assert!(
        !users_code(&draft, &stored(&draft), Some(&moved), true),
        "it replaced another script"
    );

    let global = ScriptDraft {
        scope: ScriptScope::Global,
        ..draft.clone()
    };
    let mut in_a_repo = stored(&global);
    in_a_repo.scope = ScriptScope::Global;
    assert!(
        !users_code(&global, &in_a_repo, None, false),
        "a global script with a repository"
    );

    in_a_repo.repo = None;
    assert!(users_code(&global, &in_a_repo, None, false));
}

#[test]
fn the_save_says_whether_the_script_is_new() {
    let created = saved_from(&output(
        Some(json!({ "script": summary(31), "created": true })),
        "",
        false,
    ))
    .unwrap();
    assert_eq!(created.outcome, ScriptSaveOutcome::Created);
    assert_eq!(created.id, 31);
    assert_eq!(created.script.unwrap().approval, ScriptApproval::Local);

    let replaced = saved_from(&output(
        Some(json!({ "script": summary(12), "created": false })),
        "",
        false,
    ))
    .unwrap();
    assert_eq!(replaced.outcome, ScriptSaveOutcome::Replaced);

    let bare = saved_from(&output(Some(json!({ "created": true })), "", false));
    assert!(matches!(bare, Err(BusinessError::Protocol(_))));

    let refused = saved_from(&output(None, "the script body is empty", true));
    assert!(
        matches!(refused, Err(BusinessError::Process(ref message)) if message == "script_save: the script body is empty")
    );
}
