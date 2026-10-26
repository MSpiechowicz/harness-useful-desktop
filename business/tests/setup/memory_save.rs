//! Saving a note (and refusals of a script) from the app over the pooled `harness-memory mcp` connection, against
//! the stand-in server of `fake_agent` (see `src/bin/fake_agent/memory.rs` and `memory_notes.rs`):
//! the outcomes, what goes to the server, a notes import, and servers too old to save from the
//! app. The script saves are in `memory_save_scripts.rs`.

use harness_business::BusinessError;
use harness_business::ecosystem::SaveRoute;
use harness_business::ecosystem::{NotesImportRequest, apply_notes_import, read_notes_file, save_note};
use harness_business::model::{NoteDraft, NoteKind, NoteSaveOutcome, NoteSaved, ScriptScope};
use memory_stand_in::{AGENT_APPROVAL, CURRENT, LEGACY, Setup, script};
use pretty_assertions::assert_eq;

use crate::memory_stand_in;

fn note(title: &str, scope: ScriptScope) -> NoteDraft {
    NoteDraft {
        title: title.to_owned(),
        body: "Written in the app.".to_owned(),
        kind: NoteKind::Fix,
        tags: Vec::new(),
        entities: Vec::new(),
        scope,
    }
}

#[tokio::test]
async fn a_note_is_created_then_its_title_is_taken_until_replaced_by_id() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();
    let draft = note("Retry the build", ScriptScope::Repo);

    let created = save_note(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        SaveRoute::Mcp,
        Some(&project),
        &draft,
        None,
    )
    .await
    .expect("created");
    assert_eq!(
        created,
        NoteSaved {
            outcome: NoteSaveOutcome::Created,
            id: Some(9)
        }
    );

    let again = save_note(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        SaveRoute::Mcp,
        Some(&project),
        &draft,
        None,
    )
    .await
    .expect("refused");
    assert_eq!(
        again,
        NoteSaved {
            outcome: NoteSaveOutcome::Exists,
            id: Some(9)
        },
        "the app names the repository, so a same-title note is not replaced"
    );

    let replaced = save_note(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        SaveRoute::Mcp,
        Some(&project),
        &draft,
        Some(9),
    )
    .await
    .expect("replaced by id");
    assert_eq!(replaced.outcome, NoteSaveOutcome::Updated);

    let gone = save_note(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        SaveRoute::Mcp,
        Some(&project),
        &draft,
        Some(404),
    )
    .await;
    assert!(
        matches!(gone, Err(BusinessError::NotFound(ref message)) if message == "note 404"),
        "{gone:?}"
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_new_global_note_with_a_taken_title_is_exists() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    for title in ["Prefer bun over npm", "  prefer BUN   over npm "] {
        let taken = save_note(
            &setup.env,
            &setup.scripts,
            setup.runner(),
            SaveRoute::Mcp,
            Some(&project),
            &note(title, ScriptScope::Global),
            None,
        )
        .await
        .expect("refused");
        assert_eq!(
            taken,
            NoteSaved {
                outcome: NoteSaveOutcome::Exists,
                id: Some(7)
            },
            "{title:?}: the server would rewrite the global note with that title"
        );
    }

    let new = note("Prefer bun over yarn", ScriptScope::Global);
    let created = save_note(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        SaveRoute::Mcp,
        None,
        &new,
        None,
    )
    .await
    .expect("created");
    assert_eq!(
        created,
        NoteSaved {
            outcome: NoteSaveOutcome::Created,
            id: Some(9)
        },
        "a similar title is another note"
    );

    setup.scripts.shutdown().await;
}

/// Global notes that make the search unreliable or misleading: more notes with the same words than
/// one search lists (100 to 124) ahead of `Retry flaky step` (125), a title without a word (126),
/// and a title that looks like `Plant` with entities (127).
const MANY_GLOBAL_NOTES: &str = "Retry flaky step 1\nRetry flaky step 2\nRetry flaky step 3\nRetry flaky step 4\n\
     Retry flaky step 5\nRetry flaky step 6\nRetry flaky step 7\nRetry flaky step 8\nRetry flaky step 9\n\
     Retry flaky step 10\nRetry flaky step 11\nRetry flaky step 12\nRetry flaky step 13\nRetry flaky step 14\n\
     Retry flaky step 15\nRetry flaky step 16\nRetry flaky step 17\nRetry flaky step 18\nRetry flaky step 19\n\
     Retry flaky step 20\nRetry flaky step 21\nRetry flaky step 22\nRetry flaky step 23\nRetry flaky step 24\n\
     Retry flaky step 25\nRetry flaky step\n+++ ***\nPlant {x}";

#[tokio::test]
async fn a_global_title_is_found_where_the_search_cannot_tell() {
    let setup = Setup::installed(CURRENT, &[("FAKE_MEMORY_NOTES", MANY_GLOBAL_NOTES)]);
    let save = |title: &'static str| {
        let draft = note(title, ScriptScope::Global);
        let setup = &setup;
        async move {
            save_note(
                &setup.env,
                &setup.scripts,
                setup.runner(),
                SaveRoute::Mcp,
                None,
                &draft,
                None,
            )
            .await
            .expect("answered")
        }
    };
    let exists = |id| NoteSaved {
        outcome: NoteSaveOutcome::Exists,
        id: Some(id),
    };

    assert_eq!(
        save("retry  FLAKY step").await,
        exists(125),
        "the search lists 20 other notes first"
    );
    assert_eq!(save(" +++   *** ").await, exists(126), "nothing to search for");
    assert_eq!(
        save("Pre\u{200B}fer bun over npm\u{FE0F}").await,
        exists(7),
        "the server drops invisible characters"
    );

    assert_eq!(
        save("Plant").await.outcome,
        NoteSaveOutcome::Created,
        "note 127 is titled `Plant {{x}}`, it has no entity `x`"
    );
    assert_eq!(save("--- ///").await.outcome, NoteSaveOutcome::Created);

    setup.scripts.shutdown().await;
}

/// Global rows the search cannot tell (a full search, no word to search for), two of them the
/// same new title, and a row the search finds.
const GLOBAL_ROWS: &str = "title,body,scope\n\
     Retry flaky step,Imported.,global\n\
     +++ ***,Imported.,global\n\
     --- ///,Imported.,global\n\
     Retry flaky step 3,Imported.,global\n\
     ===,Imported.,global\n\
     --- ///,Imported again.,global\n\
     Prefer bun over npm,Imported.,global\n";

#[tokio::test]
async fn an_import_lists_the_global_notes_once() {
    let setup = Setup::installed(CURRENT, &[("FAKE_MEMORY_NOTES", MANY_GLOBAL_NOTES)]);
    let read = read_notes_file("notes.csv", GLOBAL_ROWS.as_bytes(), None).expect("read");
    let request = NotesImportRequest {
        rows: (0..7).collect(),
        default_scope: ScriptScope::Global,
        replace: false,
    };

    let result = apply_notes_import(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        SaveRoute::Mcp,
        &read.stored,
        &request,
        |_| {},
    )
    .await
    .expect("imported");

    let exists: Vec<(u32, Option<i64>)> = result.exists.iter().map(|row| (row.index, row.id)).collect();
    let created_again = exists.iter().find(|(index, _)| *index == 5).and_then(|(_, id)| *id);
    assert_eq!(
        exists,
        [
            (0, Some(125)),
            (1, Some(126)),
            (3, Some(102)),
            (5, created_again),
            (6, Some(7))
        ]
    );
    assert!(created_again.is_some(), "the note row 2 created");
    assert_eq!((result.created, result.updated, result.failed.len()), (2, 0, 0));

    // A listing starts at the first page (and may read more).
    let listings = setup
        .runs()
        .iter()
        .filter(|spec| spec.args.starts_with(&["notes".into(), "list".into()]))
        .filter(|spec| spec.args.windows(2).any(|pair| pair == ["--offset", "0"]))
        .count();
    assert_eq!(listings, 1, "one listing of the global notes for the whole import");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_server_older_than_the_notes_view_cannot_save() {
    for vars in [LEGACY, AGENT_APPROVAL] {
        let setup = Setup::installed(CURRENT, vars);
        let project = setup.project();

        let note = save_note(
            &setup.env,
            &setup.scripts,
            setup.runner(),
            SaveRoute::Mcp,
            Some(&project),
            &note("Retry the build", ScriptScope::Repo),
            None,
        )
        .await;
        assert!(matches!(note, Err(BusinessError::Unsupported(_))), "{vars:?}: {note:?}");

        let script = setup
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
            .await;
        assert!(
            matches!(script, Err(BusinessError::Unsupported(_))),
            "{vars:?}: {script:?}"
        );

        setup.scripts.shutdown().await;
    }
}

#[tokio::test]
async fn without_a_server_saving_is_not_found() {
    let setup = Setup::without_server(CURRENT, &[]);

    let note = save_note(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        SaveRoute::Mcp,
        None,
        &note("Retry the build", ScriptScope::Global),
        None,
    )
    .await;
    assert!(matches!(note, Err(BusinessError::NotFound(_))), "{note:?}");
}
