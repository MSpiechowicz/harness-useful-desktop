//! The user's own saves through the commands of a server with `save-guards` (`notes save|update`,
//! `scripts save`), which agents cannot reach, and the saves it held for review (`held
//! list|save|skip`), against the stand-in server of `fake_agent` (`src/bin/fake_agent/memory_user.rs`).

use std::path::PathBuf;

use harness_business::BusinessError;
use harness_business::ecosystem::MemoryFeatures;
use harness_business::ecosystem::memory_user_save::{CHANGED_MESSAGE, GONE_MESSAGE};
use harness_business::ecosystem::{HeldSave, SaveRoute, guarded_server, list_held, save_held, save_note, skip_held};
use harness_business::model::{
    HeldSaved, HeldTool, NoteDraft, NoteKind, NoteSaveOutcome, ScriptSaveOutcome, ScriptScope,
};
use memory_stand_in::{CURRENT, Setup, script};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use crate::memory_stand_in;

const CHAT: &str = "3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b";
const HELD: &str = "6c1f3d2a-9b8e-4f7a-8c6d-5e4f3a2b1c0d";
const REVISION: &str = "5f1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f9";

/// A save of held save `held_id` as the agent sent it.
fn held_save(held_id: &str) -> HeldSave<'_> {
    HeldSave {
        held_id,
        revision: None,
        edited: None,
    }
}

fn note(title: &str) -> NoteDraft {
    NoteDraft {
        title: title.to_owned(),
        body: "Written in the app.".to_owned(),
        kind: NoteKind::Fix,
        tags: vec!["Build".to_owned()],
        entities: vec!["Ci".to_owned()],
        scope: ScriptScope::Repo,
    }
}

/// The stdin of the last command run, as JSON.
fn last_stdin(setup: &Setup) -> Value {
    let runs = setup.runs();
    let stdin = runs.last().unwrap().stdin.clone().expect("a draft on stdin");

    serde_json::from_slice(&stdin).unwrap()
}

fn last_args(setup: &Setup) -> Vec<String> {
    setup.run_args(setup.runs().len() - 1)
}

/// A stand-in server with `save-guards`, and its path as the held-save commands take it.
async fn guarded() -> (Setup, PathBuf) {
    let setup = Setup::installed(CURRENT, &[("FAKE_MEMORY_FEATURES", "save-guards")]);
    let server = guarded_server(&setup.env, setup.runner(), &MemoryFeatures::new())
        .await
        .expect("guarded");

    (setup, server)
}

#[tokio::test]
async fn a_note_is_saved_and_updated_through_the_user_commands() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();
    let save = |draft: NoteDraft, replace: Option<i64>| {
        let setup = &setup;
        let project = project.clone();
        async move {
            save_note(
                &setup.env,
                &setup.scripts,
                setup.runner(),
                SaveRoute::Cli,
                Some(&project),
                &draft,
                replace,
            )
            .await
        }
    };

    let created = save(note("Retry the build"), None).await.expect("created");
    assert_eq!((created.outcome, created.id), (NoteSaveOutcome::Created, Some(77)));
    assert_eq!(
        last_args(&setup),
        [
            "notes",
            "save",
            "--json",
            "--client",
            "harness-desktop",
            "--project",
            project.to_string_lossy().as_ref()
        ]
    );
    assert_eq!(
        last_stdin(&setup),
        json!({
            "title": "Retry the build", "body": "Written in the app.", "kind": "fix", "tags": ["build"],
            "entities": ["Ci"], "scope": "repo",
        })
    );

    let taken = save(note("Taken title"), None).await.expect("answered");
    assert_eq!((taken.outcome, taken.id), (NoteSaveOutcome::Exists, Some(3)));

    let updated = save(note("Retry the build"), Some(5)).await.expect("updated");
    assert_eq!((updated.outcome, updated.id), (NoteSaveOutcome::Updated, Some(5)));
    assert_eq!(
        last_args(&setup),
        [
            "notes",
            "update",
            "5",
            "--json",
            "--client",
            "harness-desktop",
            "--project",
            project.to_string_lossy().as_ref()
        ]
    );
    assert_eq!(
        last_stdin(&setup),
        json!({ "title": "Retry the build", "body": "Written in the app.", "kind": "fix", "tags": ["build"] })
    );

    let gone = save(note("Retry the build"), Some(404)).await;
    assert!(
        matches!(gone, Err(BusinessError::NotFound(ref text)) if text == "note 404"),
        "{gone:?}"
    );
}

#[tokio::test]
async fn a_script_is_saved_through_the_user_command_and_read_again() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let saved = setup
        .scripts
        .save(
            &setup.env,
            setup.runner(),
            SaveRoute::Cli,
            Some(&project),
            &script("open-prs"),
            true,
            None,
        )
        .await
        .expect("replaced");

    assert_eq!((saved.outcome, saved.id), (ScriptSaveOutcome::Replaced, 12));
    assert_eq!(saved.script.expect("read again").name, "open-prs");
    assert_eq!(
        last_args(&setup),
        [
            "scripts",
            "save",
            "--json",
            "--client",
            "harness-desktop",
            "--project",
            project.to_string_lossy().as_ref()
        ]
    );
    assert_eq!(
        last_stdin(&setup),
        json!({
            "scope": "repo", "name": "open-prs", "description": "Written in the app", "language": "sh",
            "body": "echo app\n", "timeoutSecs": 45,
        })
    );
}

#[tokio::test]
async fn held_saves_are_listed_saved_and_skipped_through_the_server() {
    let (setup, server) = guarded().await;

    let held = list_held(&setup.env, setup.runner(), &server, CHAT)
        .await
        .expect("listed");
    assert_eq!(held.len(), 1, "an entry of another version is skipped");
    assert_eq!((held[0].held_id.as_str(), held[0].tool), (HELD, HeldTool::NoteSave));
    assert_eq!(held[0].revision.as_deref(), Some(REVISION));
    assert_eq!(held[0].arguments["title"], "Retry the flaky build");
    assert_eq!(last_args(&setup), ["held", "list", "--json", "--chat", CHAT]);

    let edited = json!({ "title": "Retry the flaky build twice", "body": "Twice.", "kind": "fix", "tags": [] });
    let save = HeldSave {
        edited: Some(&edited),
        revision: Some(REVISION),
        ..held_save(HELD)
    };
    let saved = save_held(&setup.env, setup.runner(), &server, save)
        .await
        .expect("saved");
    assert_eq!(saved, HeldSaved::Note { id: 12 });
    assert_eq!(last_stdin(&setup), edited);
    assert_eq!(
        last_args(&setup),
        ["held", "save", HELD, "--json", "--expect", REVISION]
    );

    skip_held(&setup.env, setup.runner(), &server, HELD, None)
        .await
        .expect("skipped");
    assert_eq!(last_args(&setup), ["held", "skip", HELD, "--json"]);

    skip_held(&setup.env, setup.runner(), &server, HELD, Some(REVISION))
        .await
        .expect("skipped at the revision seen");
    assert_eq!(
        last_args(&setup),
        ["held", "skip", HELD, "--json", "--expect", REVISION]
    );

    let gone = save_held(
        &setup.env,
        setup.runner(),
        &server,
        held_save("8e3f5a4c-1d0a-4b9c-8e7f-7a6b5c4d3e2f"),
    )
    .await;
    assert!(
        matches!(gone, Err(BusinessError::Conflict(ref text)) if text == GONE_MESSAGE),
        "{gone:?}"
    );

    let other = "7d2e4f3b-0c9f-4a8b-9d7e-6f5a4b3c2d1e";
    let missing = skip_held(&setup.env, setup.runner(), &server, other, None).await;
    assert!(matches!(missing, Err(BusinessError::NotFound(_))), "{missing:?}");

    let runs = setup.runs().len();
    let bad = save_held(&setup.env, setup.runner(), &server, held_save("../escape")).await;
    assert!(matches!(bad, Err(BusinessError::InvalidInput(_))), "{bad:?}");

    let bad_revision = HeldSave {
        revision: Some("--all"),
        ..held_save(HELD)
    };
    let bad = save_held(&setup.env, setup.runner(), &server, bad_revision).await;
    assert!(matches!(bad, Err(BusinessError::InvalidInput(_))), "{bad:?}");

    let bad = skip_held(&setup.env, setup.runner(), &server, HELD, Some("--all")).await;
    assert!(matches!(bad, Err(BusinessError::InvalidInput(_))), "{bad:?}");
    assert_eq!(
        setup.runs().len(),
        runs,
        "nothing ran for an id or revision that is not one"
    );
}

#[tokio::test]
async fn a_held_save_the_agent_changed_since_the_user_saw_it_is_not_saved() {
    let (setup, server) = guarded().await;

    let seen = "0".repeat(64);
    let stale = HeldSave {
        revision: Some(&seen),
        ..held_save(HELD)
    };
    let refused = save_held(&setup.env, setup.runner(), &server, stale).await;

    assert!(
        matches!(refused, Err(BusinessError::Changed(ref text)) if text == CHANGED_MESSAGE),
        "{refused:?}"
    );
    assert_eq!(
        refused.unwrap_err().code(),
        harness_business::model::IpcErrorCode::Changed
    );
}

#[tokio::test]
async fn a_held_save_the_agent_changed_since_the_user_saw_it_is_not_skipped() {
    let (setup, server) = guarded().await;

    let seen = "0".repeat(64);
    let refused = skip_held(&setup.env, setup.runner(), &server, HELD, Some(&seen)).await;

    assert!(
        matches!(refused, Err(BusinessError::Changed(ref text)) if text == CHANGED_MESSAGE),
        "{refused:?}"
    );
    assert_eq!(
        last_args(&setup),
        ["held", "skip", HELD, "--json", "--expect", seen.as_str()]
    );
}

#[tokio::test]
async fn a_chat_holding_more_than_a_list_page_of_output_is_still_listed() {
    let (setup, server) = guarded().await;

    let held = list_held(
        &setup.env,
        setup.runner(),
        &server,
        "9a8b7c6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d",
    )
    .await
    .expect("listed in full");

    assert_eq!(held.len(), 1);
    assert_eq!(held[0].arguments["body"].as_str().map(str::len), Some(3 * 1024 * 1024));
}

#[tokio::test]
async fn a_server_without_save_guards_holds_nothing() {
    let older = Setup::installed(CURRENT, &[]);
    let refused = guarded_server(&older.env, older.runner(), &MemoryFeatures::new()).await;
    assert!(matches!(refused, Err(BusinessError::Unsupported(_))), "{refused:?}");

    let missing = Setup::without_server(CURRENT, &[]);
    let refused = guarded_server(&missing.env, missing.runner(), &MemoryFeatures::new()).await;
    assert!(matches!(refused, Err(BusinessError::NotFound(_))), "{refused:?}");
}
