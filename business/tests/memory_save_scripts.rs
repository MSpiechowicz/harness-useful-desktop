//! Saving a script from the app over the pooled `harness-memory mcp` connection, against the
//! stand-in server of `fake_agent` (see `src/bin/fake_agent/memory.rs`): a taken name, edits
//! against the version the editor opened, and when the user's code is approved.

use harness_business::BusinessError;
use harness_business::ecosystem::SaveRoute;
use harness_business::ecosystem::{Repos, script_sha256};
use harness_business::model::{
    ScriptApproval, ScriptDraft, ScriptLanguage, ScriptSaveOutcome, ScriptSaved, ScriptScope, ScriptSummary,
};
use memory_stand_in::{CURRENT, Setup, Vars, script};
use pretty_assertions::assert_eq;

mod memory_stand_in;

#[tokio::test]
async fn a_script_name_taken_in_its_scope_is_exists_unless_replacing() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let exists = setup
        .scripts
        .save(
            &setup.env,
            setup.runner(),
            SaveRoute::Mcp,
            Some(&project),
            &script("open-prs"),
            false,
            None,
        )
        .await
        .expect("refused");
    assert_eq!((exists.outcome, exists.id), (ScriptSaveOutcome::Exists, 12));

    let replaced = setup
        .scripts
        .save(
            &setup.env,
            setup.runner(),
            SaveRoute::Mcp,
            Some(&project),
            &script("open-prs"),
            true,
            None,
        )
        .await
        .expect("replaced");
    let saved = replaced.script.expect("the saved script");
    assert_eq!(replaced.outcome, ScriptSaveOutcome::Replaced);
    assert_eq!((saved.id, saved.language.as_str(), saved.timeout_secs), (12, "sh", 45));

    let refused = setup
        .scripts
        .save(
            &setup.env,
            setup.runner(),
            SaveRoute::Mcp,
            Some(&project),
            &ScriptDraft {
                body: "fail".to_owned(),
                ..script("broken")
            },
            false,
            None,
        )
        .await;
    assert!(
        matches!(refused, Err(BusinessError::Process(ref message)) if message.starts_with("script_save: ")),
        "{refused:?}"
    );

    setup.scripts.shutdown().await;
}

/// The hash of `release-notes` as the editor opened it (the stand-in's fixture hash).
const OPENED: &str = "0000000000000000000000000000000000000000000000000000000000000007";

/// The user's edit of `release-notes`, the stand-in's script waiting for approval.
fn edit_of_the_waiting_script() -> ScriptDraft {
    ScriptDraft {
        body: "git log --oneline -50\n".to_owned(),
        scope: ScriptScope::Global,
        ..script("release-notes")
    }
}

/// The `<id>=<sha256>` of every `harness-memory trust` run so far.
fn trusted(setup: &Setup) -> Vec<String> {
    setup
        .runs()
        .iter()
        .filter(|spec| spec.args.first().is_some_and(|arg| arg == "trust"))
        .flat_map(|spec| spec.args[1..].iter())
        .map(|arg| arg.to_string_lossy().into_owned())
        .filter(|arg| arg.contains('='))
        .collect()
}

#[tokio::test]
async fn the_users_edit_of_a_script_waiting_for_approval_is_approved() {
    let draft = edit_of_the_waiting_script();
    let (setup, sha) = approving(&draft, &[("FAKE_MEMORY_SEEN_UNAPPROVED", "1")]);
    let (saved, script) = edit(&setup, &draft).await;

    assert_eq!(
        (saved.outcome, script.id, script.approval, script.trusted),
        (ScriptSaveOutcome::Replaced, 7, ScriptApproval::Approved, true)
    );
    assert_eq!(trusted(&setup), [format!("7={sha}")], "the version the user wrote");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn an_edit_whose_stored_version_changed_stays_waiting_for_approval() {
    let setup = Setup::installed(CURRENT, &[("FAKE_MEMORY_SEEN_UNAPPROVED", "1")]);
    let project = setup.project();
    let draft = edit_of_the_waiting_script();

    let created = setup
        .scripts
        .save(
            &setup.env,
            setup.runner(),
            SaveRoute::Mcp,
            Some(&project),
            &script("new-one"),
            false,
            None,
        )
        .await
        .expect("created");
    assert_eq!(
        created.script.expect("the saved script").approval,
        ScriptApproval::Local
    );
    assert_eq!(
        trusted(&setup),
        Vec::<String>::new(),
        "a script saved here is trusted already"
    );

    let saved = setup
        .scripts
        .save(
            &setup.env,
            setup.runner(),
            SaveRoute::Mcp,
            Some(&project),
            &draft,
            true,
            Some(OPENED),
        )
        .await
        .expect("saved, though not approved");
    let script = saved.script.expect("the saved script");

    assert_eq!(
        (saved.outcome, script.id, script.approval, script.trusted),
        (ScriptSaveOutcome::Replaced, 7, ScriptApproval::Needed, false)
    );
    assert_eq!(
        trusted(&setup),
        [format!("7={}", script_sha256(draft.language, &draft.body))],
        "the server answered that the script changed since the user wrote it"
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn an_edit_of_a_waiting_script_that_keeps_its_body_stays_waiting_for_approval() {
    // The stand-in's `release-notes` (bash), with only its description, timeout and language
    // changed: the agent's code, not the user's.
    let draft = ScriptDraft {
        description: "Edited in the app".to_owned(),
        language: ScriptLanguage::Sh,
        body: "git log --oneline -20\n".to_owned(),
        timeout_secs: 30,
        ..edit_of_the_waiting_script()
    };
    let (setup, _) = approving(&draft, &[]);
    let (saved, script) = edit(&setup, &draft).await;

    assert_eq!(
        (saved.outcome, script.id, script.approval, script.trusted),
        (ScriptSaveOutcome::Replaced, 7, ScriptApproval::Needed, false)
    );
    assert_eq!(trusted(&setup), Vec::<String>::new(), "nothing approved");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_saved_script_that_is_not_the_draft_stays_waiting_for_approval() {
    let draft = edit_of_the_waiting_script();
    let (setup, _) = approving(
        &draft,
        &[
            ("FAKE_MEMORY_SAVED_REPO", "github.com/other/r"),
            ("FAKE_MEMORY_SEEN_UNAPPROVED", "1"),
        ],
    );
    let (_, script) = edit(&setup, &draft).await;

    assert_eq!(
        (script.id, script.approval, script.repo.as_deref()),
        (7, ScriptApproval::Needed, Some("github.com/other/r")),
        "a global draft stored with a repository"
    );
    assert_eq!(trusted(&setup), Vec::<String>::new(), "nothing approved");

    setup.scripts.shutdown().await;
}

/// A setup whose `harness-memory trust` approves `draft`'s version (so a save that asks to trust
/// it succeeds), with the stand-in's `extra` settings, and that version's hash.
fn approving(draft: &ScriptDraft, extra: Vars) -> (Setup, &'static str) {
    let sha: &'static str = Box::leak(script_sha256(draft.language, &draft.body).into_boxed_str());
    let mut vars = vec![("FAKE_MEMORY_TRUSTED_SHA", sha)];
    vars.extend_from_slice(extra);

    (Setup::installed(CURRENT, Box::leak(vars.into_boxed_slice())), sha)
}

#[tokio::test]
async fn an_edit_of_a_script_that_changed_since_it_was_opened_saves_nothing() {
    let draft = edit_of_the_waiting_script();
    let (setup, _) = approving(&draft, &[]);
    let project = setup.project();
    let other_version = "ab".repeat(32);

    let refused = setup
        .scripts
        .save(
            &setup.env,
            setup.runner(),
            SaveRoute::Mcp,
            Some(&project),
            &draft,
            true,
            Some(&other_version),
        )
        .await;
    assert!(
        matches!(refused, Err(BusinessError::Conflict(ref message)) if message == "the script changed since you opened it"),
        "{refused:?}"
    );

    let kept = setup
        .scripts
        .get(&setup.env, setup.runner(), 7, Repos::Project(Some(&project)))
        .await
        .expect("still there");
    assert_eq!(
        (kept.body.as_str(), kept.summary.approval),
        ("git log --oneline -20\n", ScriptApproval::Needed),
        "the agent's version, not saved over and not approved"
    );
    assert_eq!(trusted(&setup), Vec::<String>::new(), "nothing approved");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn an_edit_of_a_script_deleted_since_it_was_opened_saves_nothing() {
    let draft = edit_of_the_waiting_script();
    let (setup, _) = approving(&draft, &[]);
    let project = setup.project();

    setup
        .scripts
        .delete(&setup.env, setup.runner(), 7, Repos::Project(Some(&project)))
        .await
        .expect("deleted");

    let refused = setup
        .scripts
        .save(
            &setup.env,
            setup.runner(),
            SaveRoute::Mcp,
            Some(&project),
            &draft,
            true,
            Some(OPENED),
        )
        .await;
    assert!(matches!(refused, Err(BusinessError::Conflict(_))), "{refused:?}");

    let again = setup
        .scripts
        .save(
            &setup.env,
            setup.runner(),
            SaveRoute::Mcp,
            Some(&project),
            &draft,
            false,
            None,
        )
        .await
        .expect("an add");
    assert_eq!(
        again.outcome,
        ScriptSaveOutcome::Created,
        "the refused edit wrote nothing back"
    );
    assert_eq!(
        trusted(&setup),
        Vec::<String>::new(),
        "a script saved here is trusted already"
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn an_edit_of_line_endings_or_trailing_whitespace_alone_stays_waiting_for_approval() {
    for body in [
        "git log --oneline -20\r\n",
        "git log --oneline -20 \t\n\n  \n",
        "git log --oneline -20",
    ] {
        let draft = ScriptDraft {
            body: body.to_owned(),
            ..edit_of_the_waiting_script()
        };
        let (setup, _) = approving(&draft, &[]);
        let (_, script) = edit(&setup, &draft).await;

        assert_eq!(script.approval, ScriptApproval::Needed, "{body:?}");
        assert_eq!(trusted(&setup), Vec::<String>::new(), "{body:?}: nothing approved");

        setup.scripts.shutdown().await;
    }
}

#[tokio::test]
async fn replacing_a_waiting_script_with_its_own_code_keeps_it_waiting() {
    // "Replace it" on the taken name `release-notes` from a new script with the agent's code.
    let draft = ScriptDraft {
        body: "git log --oneline -20  \r\n".to_owned(),
        ..edit_of_the_waiting_script()
    };
    let (setup, _) = approving(&draft, &[]);
    let project = setup.project();

    let saved = setup
        .scripts
        .save(
            &setup.env,
            setup.runner(),
            SaveRoute::Mcp,
            Some(&project),
            &draft,
            true,
            None,
        )
        .await
        .expect("replaced");

    assert_eq!(saved.script.expect("the saved script").approval, ScriptApproval::Needed);
    assert_eq!(trusted(&setup), Vec::<String>::new(), "nothing approved");

    setup.scripts.shutdown().await;
}

/// Saves `draft` as an edit of the version the editor opened: the answer and the saved script.
async fn edit(setup: &Setup, draft: &ScriptDraft) -> (ScriptSaved, ScriptSummary) {
    let project = setup.project();
    let saved = setup
        .scripts
        .save(
            &setup.env,
            setup.runner(),
            SaveRoute::Mcp,
            Some(&project),
            draft,
            true,
            Some(OPENED),
        )
        .await
        .expect("replaced");
    let script = saved.script.clone().expect("the saved script");

    (saved, script)
}
