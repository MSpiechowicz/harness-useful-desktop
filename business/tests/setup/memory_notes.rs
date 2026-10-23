//! The agents' notes through `harness-memory notes list|get|delete`, against the stand-in of
//! `fake_agent` (see `src/bin/fake_agent/memory_cli.rs`): the page, search, kind, sort and order on
//! the command line, deleting them all, the project's repository notes only with the project, links both ways, a note the
//! server cannot see as `notFound`, failures redacted, and older servers as `outdated`.

use std::ffi::OsString;
use std::path::Path;

use harness_business::BusinessError;
use harness_business::ecosystem::memory_cli::{MAX_PAGE_SIZE, MAX_QUERY_CHARS};
use harness_business::ecosystem::{ListWindow, NotesQuery, Repos, delete_all_notes, delete_note, get_note, list_notes};
use harness_business::model::{
    MemoryDeleteAll, MemoryNotesPage, MemoryScriptsAvailability, NoteKind, NoteKindCount, NoteLink, NoteLinkDirection,
    NoteSort, ScriptPlace, ScriptScope, SortOrder,
};
use memory_stand_in::{AGENT_APPROVAL, CURRENT, LEGACY, Setup, UNPAGED};
use pretty_assertions::assert_eq;

use crate::memory_stand_in;

fn first_page() -> NotesQuery {
    NotesQuery {
        window: ListWindow::new(None, None, None, None).unwrap(),
        kind: None,
        scope: None,
        place: None,
        lang: None,
        sort: None,
        hidden: false,
    }
}

async fn list(setup: &Setup, project: Option<&Path>, query: &NotesQuery) -> MemoryNotesPage {
    list_notes(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        Repos::Project(project),
        query,
    )
    .await
}

fn ids(page: &MemoryNotesPage) -> Vec<i64> {
    page.notes.iter().map(|note| note.id).collect()
}

fn texts(args: &[OsString]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

#[tokio::test]
async fn the_first_page_holds_the_project_and_global_notes_newest_first() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let page = list(&setup, Some(&project), &first_page()).await;
    assert_eq!(page.availability, MemoryScriptsAvailability::Available);
    assert_eq!(
        ids(&page),
        [1, 2, 3, 4, 5, 6, 7],
        "the note of an unknown kind is skipped"
    );
    assert_eq!(page.total, 8, "the server's count");

    let kinds: Vec<NoteKind> = page.kinds.iter().map(|count| count.kind).collect();
    assert_eq!(
        kinds,
        [
            NoteKind::Decision,
            NoteKind::Fix,
            NoteKind::Problem,
            NoteKind::Outcome,
            NoteKind::Optimisation,
            NoteKind::Location,
            NoteKind::Note
        ],
        "the count of an unknown kind is skipped"
    );

    let fix = &page.notes[1];
    assert_eq!(fix.kind, NoteKind::Fix);
    assert_eq!(fix.scope, ScriptScope::Repo);
    assert_eq!(fix.repo.as_deref(), Some("github.com/o/r"));
    assert_eq!(fix.place, ScriptPlace::Team);
    assert_eq!(fix.tags, ["fix", "fixture"]);
    assert_eq!(fix.entities, ["src/fix.rs"]);
    assert_eq!(fix.author.source, "claude-code");
    assert_eq!((fix.revision, fix.hits), (3, 4));

    let wire = serde_json::to_value(fix).unwrap();
    assert_eq!(wire["updatedAt"], "2026-10-08T10:00:00Z");
    assert!(wire.get("body").is_none(), "a list has no bodies");

    let project_text = project.to_string_lossy();
    setup.assert_first_run(&[
        "notes",
        "list",
        "--json",
        "--offset",
        "0",
        "--limit",
        "50",
        "--sort",
        "updated",
        "--order",
        "desc",
        "--project",
        &project_text,
    ]);

    let global = list(&setup, None, &first_page()).await;
    assert_eq!(ids(&global), [1, 4, 6, 7], "global only without a project");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn search_kind_order_and_page_narrow_the_list() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let fixes = NotesQuery {
        window: ListWindow::new(None, Some(10), Some(" Watcher ".to_owned()), None).unwrap(),
        kind: Some(NoteKind::Fix),
        scope: None,
        place: None,
        lang: None,
        sort: None,
        hidden: false,
    };
    let page = list(&setup, Some(&project), &fixes).await;
    assert_eq!(ids(&page), [2]);
    assert_eq!(page.total, 1);
    assert_eq!(
        page.kinds,
        [
            NoteKindCount {
                kind: NoteKind::Fix,
                count: 1
            },
            NoteKindCount {
                kind: NoteKind::Problem,
                count: 1
            }
        ],
        "every kind under the query, whatever kind was asked for"
    );
    assert_eq!(
        texts(&setup.runs()[0].args)[3..9],
        ["--offset", "0", "--limit", "10", "--query", "Watcher"]
    );
    assert_eq!(texts(&setup.runs()[0].args)[9..11], ["--kind", "fix"]);

    let by_title = NotesQuery {
        window: ListWindow::new(Some(1), Some(2), Some("   ".to_owned()), None).unwrap(),
        kind: None,
        scope: None,
        place: None,
        lang: None,
        sort: Some(NoteSort::Title),
        hidden: false,
    };
    let page = list(&setup, None, &by_title).await;
    assert_eq!(
        ids(&page),
        [6, 7],
        "A kind…, Memory…, Prefer…, Release…, Use…: the second and third"
    );
    assert_eq!(page.total, 5);
    assert_eq!(
        texts(&setup.runs()[1].args)[3..],
        ["--offset", "1", "--limit", "2", "--sort", "title", "--order", "asc"],
        "a blank query is none, and titles go A to Z"
    );

    let newest_last = NotesQuery {
        window: ListWindow::new(None, None, None, Some(SortOrder::Asc)).unwrap(),
        kind: None,
        scope: None,
        place: None,
        lang: None,
        sort: Some(NoteSort::Updated),
        hidden: false,
    };
    assert_eq!(ids(&list(&setup, None, &newest_last).await), [7, 6, 4, 1]);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn every_sort_goes_to_the_server_by_its_name() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let sorted = |sort| NotesQuery {
        window: ListWindow::new(None, None, None, None).unwrap(),
        kind: None,
        scope: None,
        place: None,
        lang: None,
        sort: Some(sort),
        hidden: false,
    };

    let cases = [
        (NoteSort::Kind, "kind", vec![1, 6, 7, 4], None),
        (NoteSort::Scope, "scope", vec![1, 4, 6, 7, 2, 3, 5], Some(&project)),
        (NoteSort::Place, "place", vec![3, 5, 6, 7, 1, 2, 4], Some(&project)),
    ];

    for (run, (sort, name, expected, project)) in cases.into_iter().enumerate() {
        let page = list(&setup, project.map(|path| path.as_path()), &sorted(sort)).await;
        assert_eq!(ids(&page), expected, "{name}");
        assert_eq!(
            texts(&setup.runs()[run].args)[7..11],
            ["--sort", name, "--order", "asc"],
            "{name} goes A to Z unless asked"
        );
    }

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn deleting_them_all_sends_the_confirmation_and_reads_the_counts() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let deleted = delete_all_notes(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        Repos::Project(Some(&project)),
        None,
    )
    .await
    .expect("deleted");
    assert_eq!(
        deleted,
        MemoryDeleteAll {
            deleted: 8,
            team: Some(3),
            kept: None,
        }
    );
    assert_eq!(
        texts(&setup.runs()[0].args),
        [
            "notes",
            "delete",
            "--all",
            "--yes",
            "--json",
            "--project",
            &project.to_string_lossy()
        ]
    );

    let global = delete_all_notes(&setup.env, &setup.scripts, setup.runner(), Repos::Project(None), None)
        .await
        .expect("deleted");
    assert_eq!(global.deleted, 5, "without a project only the global notes");

    setup.scripts.shutdown().await;

    let older = Setup::installed(CURRENT, UNPAGED);
    let refused = delete_all_notes(&older.env, &older.scripts, older.runner(), Repos::Project(None), None).await;
    assert!(matches!(refused, Err(BusinessError::Unsupported(_))), "{refused:?}");
    older.scripts.shutdown().await;

    let missing = Setup::without_server(CURRENT, &[]);
    let refused = delete_all_notes(
        &missing.env,
        &missing.scripts,
        missing.runner(),
        Repos::Project(None),
        None,
    )
    .await;
    assert!(matches!(refused, Err(BusinessError::NotFound(_))), "{refused:?}");
    assert!(missing.requested().is_empty());
}

#[test]
fn a_page_holds_one_to_two_hundred_notes_and_a_search_is_short() {
    let window = |limit, query: &str| ListWindow::new(None, limit, Some(query.to_owned()), None);

    assert!(window(Some(1), "").is_ok());
    assert!(window(Some(MAX_PAGE_SIZE), &"a".repeat(MAX_QUERY_CHARS)).is_ok());

    for result in [
        window(Some(0), ""),
        window(Some(MAX_PAGE_SIZE + 1), ""),
        window(None, &"é".repeat(MAX_QUERY_CHARS + 1)),
    ] {
        assert!(matches!(result, Err(BusinessError::InvalidInput(_))), "{result:?}");
    }
}

#[tokio::test]
async fn a_note_comes_with_its_body_and_the_links_both_ways() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let detail = get_note(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        2,
        Repos::Project(Some(&project)),
        false,
    )
    .await
    .expect("note 2");
    assert_eq!(detail.note.summary.title, "Retry the watcher on rename");
    assert_eq!(detail.note.body, "Retry the watcher on rename.\nWritten by an agent.\n");
    assert_eq!(
        detail.links,
        [
            NoteLink {
                id: 3,
                title: "Watcher misses renames".to_owned(),
                kind: NoteKind::Problem,
                relation: "fixes".to_owned(),
                direction: NoteLinkDirection::Out,
            },
            NoteLink {
                id: 5,
                title: "Cache the tools probe".to_owned(),
                kind: NoteKind::Optimisation,
                relation: "follows".to_owned(),
                direction: NoteLinkDirection::In,
            }
        ]
    );

    let wire = serde_json::to_value(&detail).unwrap();
    assert_eq!(wire["note"]["body"], detail.note.body);
    assert_eq!(wire["note"]["kind"], "fix", "the summary is flattened");
    assert_eq!(wire["links"][1]["direction"], "in");

    assert_eq!(
        texts(&setup.runs()[0].args),
        ["notes", "get", "2", "--json", "--project", &project.to_string_lossy()]
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_note_the_server_cannot_see_is_not_found() {
    let setup = Setup::installed(CURRENT, &[]);

    let repository_note = get_note(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        3,
        Repos::Project(None),
        false,
    )
    .await;
    assert!(
        matches!(&repository_note, Err(BusinessError::NotFound(message)) if message == "note 3"),
        "{repository_note:?}"
    );

    let unknown = delete_note(&setup.env, &setup.scripts, setup.runner(), 42, Repos::Project(None)).await;
    assert!(matches!(unknown, Err(BusinessError::NotFound(_))), "{unknown:?}");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn delete_runs_on_the_project_and_failures_come_back_redacted() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    delete_note(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        3,
        Repos::Project(Some(&project)),
    )
    .await
    .expect("deleted");
    assert_eq!(
        texts(&setup.runs()[0].args),
        ["notes", "delete", "3", "--project", &project.to_string_lossy()]
    );

    for result in [
        get_note(
            &setup.env,
            &setup.scripts,
            setup.runner(),
            500,
            Repos::Project(None),
            false,
        )
        .await
        .map(|_| ()),
        delete_note(&setup.env, &setup.scripts, setup.runner(), 500, Repos::Project(None)).await,
    ] {
        let Err(BusinessError::Process(message)) = &result else {
            panic!("expected a process error, got {result:?}");
        };

        assert!(
            message.starts_with("harness-memory: the team database cannot be reached"),
            "the server's last line, not its warning: {message}"
        );
        assert!(!message.contains("hunter2"), "{message}");
    }

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn older_servers_are_outdated() {
    for vars in [LEGACY, AGENT_APPROVAL, UNPAGED] {
        let setup = Setup::installed(CURRENT, vars);

        let page = list(&setup, None, &first_page()).await;
        assert_eq!(page.availability, MemoryScriptsAvailability::Outdated, "{vars:?}");
        assert!(page.notes.is_empty() && page.kinds.is_empty() && page.total == 0);

        let get = get_note(
            &setup.env,
            &setup.scripts,
            setup.runner(),
            7,
            Repos::Project(None),
            false,
        )
        .await;
        assert!(matches!(get, Err(BusinessError::Unsupported(_))), "{vars:?}: {get:?}");

        setup.scripts.shutdown().await;
    }
}

#[tokio::test]
async fn without_the_binary_nothing_runs() {
    let setup = Setup::without_server(CURRENT, &[]);

    let page = list(&setup, None, &first_page()).await;
    assert_eq!(page.availability, MemoryScriptsAvailability::NotInstalled);

    let delete = delete_note(&setup.env, &setup.scripts, setup.runner(), 7, Repos::Project(None)).await;
    assert!(matches!(delete, Err(BusinessError::NotFound(_))), "{delete:?}");
    assert!(setup.requested().is_empty());
}

#[tokio::test]
async fn a_server_that_does_not_start_fails_the_page() {
    // `fake_agent` without a dialect exits at once with its usage.
    let setup = Setup::installed(&[], &[]);

    let MemoryScriptsAvailability::Failed { message } = list(&setup, None, &first_page()).await.availability else {
        panic!("expected failed");
    };
    assert!(message.contains("exited before answering initialize"), "{message}");
}
