//! Who wrote a note or a script (`notes-authors`), against the stand-in of `fake_agent`: the
//! authors on the rows of a server that lists the feature, rows without them from one that does
//! not, and the features the app reads.

use harness_business::ecosystem::{ListWindow, MemoryFeatures, NotesQuery, Repos, ScriptsQuery, list_notes};
use harness_business::model::{MemoryNotesPage, MemoryScriptsPage, NoteSummary, ScriptSummary};
use memory_stand_in::{CURRENT, Setup};
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::memory_stand_in;

/// A server that predates `notes-authors`.
const WITHOUT_AUTHORS: &[(&str, &str)] = &[("FAKE_MEMORY_FEATURES", "notes-hide,notes-lang,notes-useful")];

async fn notes(setup: &Setup) -> MemoryNotesPage {
    let query = NotesQuery {
        window: ListWindow::new(None, None, None, None).unwrap(),
        kind: None,
        scope: None,
        place: None,
        lang: None,
        useful: None,
        sort: None,
        hidden: false,
    };

    list_notes(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        Repos::Project(Some(&setup.project())),
        &query,
    )
    .await
}

async fn scripts(setup: &Setup) -> MemoryScriptsPage {
    let query = ScriptsQuery {
        window: ListWindow::new(None, None, None, None).unwrap(),
        group: None,
        scope: None,
        review: None,
        place: None,
        sort: None,
        hidden: false,
    };

    setup
        .scripts
        .list(
            &setup.env,
            setup.runner(),
            Repos::Project(Some(&setup.project())),
            &query,
        )
        .await
}

fn note(page: &MemoryNotesPage, id: i64) -> &NoteSummary {
    page.notes.iter().find(|note| note.id == id).expect("a note")
}

fn script(page: &MemoryScriptsPage, id: i64) -> &ScriptSummary {
    page.scripts.iter().find(|script| script.id == id).expect("a script")
}

#[tokio::test]
async fn notes_and_scripts_carry_who_added_and_changed_them() {
    let setup = Setup::installed(CURRENT, &[]);
    let features = MemoryFeatures::new().support(&setup.env, setup.runner()).await;
    assert!(features.notes_authors());

    let page = notes(&setup).await;
    let agent = note(&page, 2);
    let by = agent.added_by.as_ref().expect("added by");
    assert_eq!(
        (by.name.as_str(), by.via.as_deref()),
        ("Ada Lovelace", Some("claude-code"))
    );
    assert_eq!(agent.updated_by, agent.added_by);
    assert_eq!(
        agent.author.source, "claude-code",
        "the author object of the note is unchanged"
    );

    let user = note(&page, 7);
    let (added, updated) = (user.added_by.as_ref().unwrap(), user.updated_by.as_ref().unwrap());
    assert_eq!((added.name.as_str(), added.via.as_deref()), ("Ada Lovelace", None));
    assert_eq!((updated.name.as_str(), updated.via.as_deref()), ("Grace Hopper", None));

    assert_eq!(
        note(&page, 4).added_by.as_ref().map(|by| by.name.as_str()),
        Some("Ada Lovelace")
    );
    assert_eq!(
        note(&page, 1).updated_by.as_ref().map(|by| by.name.as_str()),
        Some("Grace Hopper")
    );
    assert_eq!(
        (note(&page, 3).added_by.as_ref(), note(&page, 3).updated_by.as_ref()),
        (None, None),
        "no git name was known"
    );

    let page = scripts(&setup).await;
    assert_eq!(
        script(&page, 7).added_by.as_ref().map(|by| by.name.as_str()),
        Some("Ada Lovelace")
    );
    assert_eq!(
        script(&page, 7).updated_by.as_ref().map(|by| by.name.as_str()),
        Some("Grace Hopper")
    );
    assert_eq!(script(&page, 12).added_by, None, "no git name was known");
    assert_eq!(script(&page, 7).author.source, "claude-code");
}

#[tokio::test]
async fn a_server_without_notes_authors_sends_none_and_its_rows_still_read() {
    let setup = Setup::installed(CURRENT, WITHOUT_AUTHORS);
    let features = MemoryFeatures::new().support(&setup.env, setup.runner()).await;
    assert!(!features.notes_authors());
    assert!(features.notes_useful(), "the other features are kept");

    let page = notes(&setup).await;
    assert!(!page.notes.is_empty());
    assert!(
        page.notes
            .iter()
            .all(|note| note.added_by.is_none() && note.updated_by.is_none())
    );

    let page = scripts(&setup).await;
    assert!(!page.scripts.is_empty());
    assert!(
        page.scripts
            .iter()
            .all(|script| script.added_by.is_none() && script.updated_by.is_none())
    );
}

#[test]
fn an_author_on_the_wire_is_cleaned_and_one_unreadable_is_none() {
    let mut row = json!({
        "id": 1, "kind": "note", "title": "t", "scope": "global", "repo": null, "place": "local",
        "tags": [], "entities": [], "author": { "source": "codex", "root": null },
        "createdAt": "2026-10-01T08:00:00Z", "updatedAt": "2026-10-01T08:00:00Z",
        "revision": 1, "hits": 0,
        "addedBy": { "name": "Ada\u{202e}\u{0000}", "via": "codex" },
        "updatedBy": 7
    });

    let note: NoteSummary = serde_json::from_value(row.clone()).unwrap();
    assert_eq!(note.added_by.as_ref().map(|by| by.name.as_str()), Some("Ada"));
    assert_eq!(note.updated_by, None);

    row.as_object_mut().unwrap().remove("addedBy");
    row.as_object_mut().unwrap().remove("updatedBy");
    let note: NoteSummary = serde_json::from_value(row).unwrap();
    assert_eq!(
        (note.added_by, note.updated_by),
        (None, None),
        "an older server sends neither"
    );
}
