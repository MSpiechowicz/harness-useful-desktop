//! Team notes beside the notes saved on this computer, through `harness-memory notes`, against the
//! stand-in of `fake_agent` (see `src/bin/fake_agent/memory_push.rs`): read-only team notes in a
//! scope routed `remote+local`, the hidden count and the local counts of a page, the hidden team
//! notes (`--hidden`), hiding and showing one, deleting them all while team notes stay, discarding
//! local notes (ids on stdin), and saving a global note beside a read-only team note.

use harness_business::BusinessError;
use harness_business::ecosystem::{
    ListWindow, NotesQuery, delete_all_notes, discard_local, get_note, hide_entry, list_notes, save_note,
};
use harness_business::model::{
    LocalNoteIds, MemoryCopyKind, MemoryDeleteAll, MemoryScopeCounts, NoteDraft, NoteIdsWord, NoteKind,
    NoteSaveOutcome, ScriptScope,
};
use memory_stand_in::{CURRENT, Setup, Vars};
use pretty_assertions::assert_eq;

mod memory_stand_in;

/// Hiding and discarding take notes.
const NOTES: MemoryCopyKind = MemoryCopyKind::Notes;

/// Global notes routed `remote+local`, as the server reads its environment.
const GLOBAL_TEAM_AND_LOCAL: Vars = &[("HARNESS_MEMORY_GLOBAL_NOTES", "remote+local")];

fn query(hidden: bool) -> NotesQuery {
    NotesQuery {
        window: ListWindow::new(None, None, None, None).unwrap(),
        kind: None,
        scope: None,
        sort: None,
        hidden,
    }
}

#[tokio::test]
async fn team_notes_of_a_scope_with_local_saves_are_read_only_and_the_page_counts_the_rest() {
    let setup = Setup::installed(CURRENT, GLOBAL_TEAM_AND_LOCAL);
    let project = setup.project();

    let page = list_notes(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        Some(&project),
        &query(false),
    )
    .await;
    let read_only: Vec<i64> = page
        .notes
        .iter()
        .filter(|note| note.read_only)
        .map(|note| note.id)
        .collect();
    assert_eq!(
        read_only,
        [1, 4],
        "the global team notes, not the repository's team note 2"
    );
    assert!(page.notes.iter().all(|note| !note.hidden));
    assert_eq!(page.hidden_count, Some(1));
    assert_eq!(page.local_counts, Some(MemoryScopeCounts { repo: 4, global: 3 }));

    let detail = get_note(&setup.env, &setup.scripts, setup.runner(), 1, None, false)
        .await
        .expect("note 1");
    assert!(detail.note.summary.read_only);

    let wire = serde_json::to_value(&page).unwrap();
    assert_eq!(wire["hiddenCount"], 1);
    assert_eq!(wire["localCounts"]["global"], 3);
    assert_eq!(wire["notes"][0]["readOnly"], true);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn hidden_team_notes_are_listed_and_read_only_when_asked_for() {
    let setup = Setup::installed(CURRENT, &[]);

    let page = list_notes(&setup.env, &setup.scripts, setup.runner(), None, &query(true)).await;
    let ids: Vec<i64> = page.notes.iter().map(|note| note.id).collect();
    assert_eq!(ids, [40]);
    assert!(page.notes[0].hidden);
    assert_eq!(setup.run_args(0).last().map(String::as_str), Some("--hidden"));

    let hidden = get_note(&setup.env, &setup.scripts, setup.runner(), 40, None, true)
        .await
        .expect("the hidden note");
    assert!(hidden.note.summary.hidden);
    assert_eq!(setup.run_args(1), ["notes", "get", "40", "--json", "--hidden"]);

    let not_hidden = get_note(&setup.env, &setup.scripts, setup.runner(), 40, None, false).await;
    assert!(matches!(not_hidden, Err(BusinessError::NotFound(_))), "{not_hidden:?}");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_team_note_is_hidden_and_shown_again_but_a_local_one_is_refused() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    hide_entry(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        NOTES,
        2,
        Some(&project),
        true,
    )
    .await
    .expect("hidden");
    hide_entry(&setup.env, &setup.scripts, setup.runner(), NOTES, 40, None, false)
        .await
        .expect("shown again");
    assert_eq!(
        setup.run_args(0),
        ["notes", "hide", "2", "--json", "--project", &project.to_string_lossy()]
    );
    assert_eq!(setup.run_args(1), ["notes", "unhide", "40", "--json"]);

    let local = hide_entry(&setup.env, &setup.scripts, setup.runner(), NOTES, 7, None, true).await;
    assert!(
        matches!(&local, Err(BusinessError::Process(message)) if message.ends_with("delete it instead")),
        "{local:?}"
    );

    let gone = hide_entry(&setup.env, &setup.scripts, setup.runner(), NOTES, 2, None, true).await;
    assert!(
        matches!(&gone, Err(BusinessError::NotFound(message)) if message == "note 2"),
        "a repository note without its project: {gone:?}"
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn deleting_them_all_keeps_the_read_only_team_notes() {
    let setup = Setup::installed(CURRENT, GLOBAL_TEAM_AND_LOCAL);

    let deleted = delete_all_notes(&setup.env, &setup.scripts, setup.runner(), None)
        .await
        .expect("deleted");
    assert_eq!(
        deleted,
        MemoryDeleteAll {
            deleted: 3,
            team: Some(0),
            kept: Some(2),
        }
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn local_notes_are_discarded_all_or_by_id_on_stdin() {
    let setup = Setup::installed(CURRENT, &[]);
    let all = LocalNoteIds::Word(NoteIdsWord::All);

    let deleted = discard_local(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        NOTES,
        &all,
        Some(ScriptScope::Global),
    )
    .await
    .expect("discarded");
    assert_eq!(deleted, 3);
    assert_eq!(
        setup.run_args(0),
        [
            "notes",
            "discard-local",
            "--all",
            "--scope",
            "global",
            "--yes",
            "--json"
        ]
    );
    assert_eq!(setup.runs()[0].stdin, None);

    let ids = LocalNoteIds::Ids(vec![3, 4]);
    let deleted = discard_local(&setup.env, &setup.scripts, setup.runner(), NOTES, &ids, None)
        .await
        .expect("discarded");
    assert_eq!(deleted, 2);
    assert_eq!(setup.run_args(1), ["notes", "discard-local", "-", "--yes", "--json"]);
    assert_eq!(setup.runs()[1].stdin.as_deref(), Some(b"3,4\n".as_slice()));

    for refused in [
        LocalNoteIds::Word(NoteIdsWord::None),
        LocalNoteIds::Ids(Vec::new()),
        LocalNoteIds::Ids(vec![3, 0]),
    ] {
        let result = discard_local(&setup.env, &setup.scripts, setup.runner(), NOTES, &refused, None).await;
        assert!(
            matches!(result, Err(BusinessError::InvalidInput(_))),
            "{refused:?}: {result:?}"
        );
    }
    assert_eq!(setup.runs().len(), 2, "nothing runs for a refused selection");

    let wire: LocalNoteIds = serde_json::from_str("[3, 4]").unwrap();
    assert_eq!(wire, LocalNoteIds::Ids(vec![3, 4]));
    let wire: LocalNoteIds = serde_json::from_str("\"all\"").unwrap();
    assert_eq!(wire, all);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_global_note_is_saved_beside_a_read_only_team_note_of_its_title() {
    let draft = NoteDraft {
        title: "Use rustls everywhere".to_owned(),
        body: "Written in the app.".to_owned(),
        kind: NoteKind::Decision,
        tags: Vec::new(),
        entities: Vec::new(),
        scope: ScriptScope::Global,
    };

    let setup = Setup::installed(CURRENT, GLOBAL_TEAM_AND_LOCAL);
    let saved = save_note(&setup.env, &setup.scripts, setup.runner(), None, &draft, None)
        .await
        .expect("saved");
    assert_eq!(saved.outcome, NoteSaveOutcome::Created, "the team note does not count");
    setup.scripts.shutdown().await;

    let setup = Setup::installed(CURRENT, &[]);
    let saved = save_note(&setup.env, &setup.scripts, setup.runner(), None, &draft, None)
        .await
        .expect("refused");
    assert_eq!(
        (saved.outcome, saved.id),
        (NoteSaveOutcome::Exists, Some(1)),
        "an editable team note does"
    );
    setup.scripts.shutdown().await;
}
