//! The usefulness of notes in the list (`notes-useful`), against the stand-in of `fake_agent`: the
//! sorts and the filter on the command line, a server without the feature as the app asks it, rows
//! with and without the counts, and one kept page per filter.

use std::path::Path;

use harness_business::ecosystem::memory_list_cache::list_notes_cached;
use harness_business::ecosystem::{ListWindow, MemoryFeatures, NotesQuery, Repos, list_notes};
use harness_business::model::{MemoryNotesPage, MemoryScriptsAvailability, NoteSort, NoteSummary, NoteUsefulFilter};
use memory_stand_in::{CURRENT, Setup};
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::memory_stand_in;

/// A server that predates `notes-useful`.
const WITHOUT_USEFUL: &[(&str, &str)] = &[("FAKE_MEMORY_FEATURES", "notes-hide,notes-lang")];

fn query(useful: Option<NoteUsefulFilter>, sort: Option<NoteSort>) -> NotesQuery {
    NotesQuery {
        window: ListWindow::new(None, None, None, None).unwrap(),
        kind: None,
        scope: None,
        place: None,
        lang: None,
        useful,
        sort,
        hidden: false,
    }
}

async fn list(setup: &Setup, project: &Path, query: &NotesQuery) -> MemoryNotesPage {
    list_notes(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        Repos::Project(Some(project)),
        query,
    )
    .await
}

fn ids(page: &MemoryNotesPage) -> Vec<i64> {
    page.notes.iter().map(|note| note.id).collect()
}

/// The arguments of the command run last.
fn last_args(setup: &Setup) -> Vec<String> {
    setup.run_args(setup.runs().len() - 1)
}

/// The value after `option` in `args`.
fn value_of<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    let at = args.iter().position(|arg| arg == option)?;

    args.get(at + 1).map(String::as_str)
}

#[tokio::test]
async fn the_useful_sorts_and_filter_go_to_a_server_that_lists_them() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let most = list(&setup, &project, &query(None, Some(NoteSort::Useful))).await;
    let args = last_args(&setup);
    assert_eq!(value_of(&args, "--sort"), Some("useful"));
    assert_eq!(value_of(&args, "--order"), Some("desc"), "most useful first by default");
    assert_eq!(&ids(&most)[..4], [1, 7, 2, 4]);
    assert_eq!(
        (most.notes[0].shown, most.notes[0].useful),
        (Some(12), Some(9)),
        "the counts come with the row"
    );
    assert_eq!(most.notes[0].last_useful_at.as_deref(), Some("2026-10-09T09:00:00Z"));

    let latest = list(&setup, &project, &query(None, Some(NoteSort::LastUseful))).await;
    let args = last_args(&setup);
    assert_eq!(value_of(&args, "--sort"), Some("lastUseful"));
    assert_eq!(value_of(&args, "--order"), Some("desc"));
    assert_eq!(&ids(&latest)[..4], [1, 2, 7, 4], "notes never used come last");

    let ever = list(&setup, &project, &query(Some(NoteUsefulFilter::Ever), None)).await;
    let args = last_args(&setup);
    assert_eq!(value_of(&args, "--useful"), Some("ever"));
    assert_eq!(value_of(&args, "--sort"), Some("updated"));
    assert_eq!(ids(&ever), [1, 2, 4, 7]);

    let never = list(&setup, &project, &query(Some(NoteUsefulFilter::Never), None)).await;
    assert_eq!(value_of(&last_args(&setup), "--useful"), Some("never"));
    assert_eq!(
        ids(&never),
        [3, 5],
        "shown at least three times and never useful (note 8 is of a kind the app skips)"
    );

    let plain = list(&setup, &project, &query(None, None)).await;
    assert!(!last_args(&setup).contains(&"--useful".to_owned()));
    assert_eq!(plain.total, 8);
}

#[tokio::test]
async fn a_server_without_the_feature_is_asked_for_neither_the_filter_nor_the_sort() {
    let setup = Setup::installed(CURRENT, WITHOUT_USEFUL);
    let project = setup.project();
    let asked = query(Some(NoteUsefulFilter::Never), Some(NoteSort::Useful));

    let refused = list(&setup, &project, &asked).await;
    assert_eq!(
        refused.availability,
        MemoryScriptsAvailability::Outdated,
        "the usage error of an older server, hence the gate"
    );

    let features = MemoryFeatures::new().support(&setup.env, setup.runner()).await;
    assert!(!features.notes_useful());
    assert!(features.notes_lang(), "the other features are kept");

    let gated = asked.for_server(features);
    assert_eq!(gated, query(None, None));
    assert!(!gated.needs_features());

    let page = list(&setup, &project, &gated).await;
    let args = last_args(&setup);
    assert_eq!(page.availability, MemoryScriptsAvailability::Available);
    assert_eq!(page.total, 8);
    assert_eq!(value_of(&args, "--sort"), Some("updated"));
    assert!(!args.contains(&"--useful".to_owned()));
    assert!(
        page.notes
            .iter()
            .all(|note| note.shown.is_none() && note.useful.is_none() && note.last_useful_at.is_none()),
        "an older server sends no counts"
    );
}

#[tokio::test]
async fn the_gate_keeps_the_other_sorts_and_filters_and_everything_for_a_server_with_the_feature() {
    let setup = Setup::installed(CURRENT, &[]);
    let features = MemoryFeatures::new().support(&setup.env, setup.runner()).await;
    assert!(features.notes_useful());

    let asked = query(Some(NoteUsefulFilter::Ever), Some(NoteSort::LastUseful));
    assert_eq!(asked.for_server(features), asked);

    let without = Setup::installed(CURRENT, WITHOUT_USEFUL);
    let older = MemoryFeatures::new().support(&without.env, without.runner()).await;
    let title = query(None, Some(NoteSort::Title));

    assert_eq!(title.for_server(older), title, "a sort the old server knows stays");
    assert!(!title.needs_features());
    assert!(asked.needs_features());
}

#[test]
fn a_note_reads_with_and_without_the_usefulness_fields() {
    let row = json!({
        "id": 7,
        "kind": "note",
        "title": "Prefer bun over npm",
        "scope": "global",
        "repo": null,
        "place": "local",
        "tags": [],
        "entities": [],
        "author": { "source": "claude-code", "root": null },
        "createdAt": "2026-10-01T08:00:00Z",
        "updatedAt": "2026-10-03T10:00:00Z",
        "revision": 2,
        "hits": 14,
    });

    let old: NoteSummary = serde_json::from_value(row.clone()).unwrap();
    assert_eq!((old.shown, old.useful, old.last_useful_at), (None, None, None));

    let mut counted = row.clone();
    counted["shown"] = json!(9);
    counted["useful"] = json!(7);
    counted["lastUsefulAt"] = json!("2026-10-03T09:00:00Z");
    let new: NoteSummary = serde_json::from_value(counted).unwrap();
    assert_eq!((new.shown, new.useful), (Some(9), Some(7)));
    assert_eq!(new.last_useful_at.as_deref(), Some("2026-10-03T09:00:00Z"));

    let mut unused = row;
    unused["shown"] = json!(0);
    unused["useful"] = json!(0);
    unused["lastUsefulAt"] = json!(null);
    let never: NoteSummary = serde_json::from_value(unused).unwrap();
    assert_eq!(
        (never.shown, never.useful, never.last_useful_at),
        (Some(0), Some(0), None)
    );
}

#[tokio::test]
async fn the_kept_page_is_per_useful_filter() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();
    let config = tempfile::tempdir().unwrap();
    let cached = |query: NotesQuery| {
        let (config, setup, project) = (config.path(), &setup, &project);

        async move {
            list_notes_cached(
                config,
                &setup.env,
                &setup.scripts,
                setup.runner(),
                Repos::Project(Some(project)),
                &query,
            )
            .await
        }
    };

    let ever = query(Some(NoteUsefulFilter::Ever), None);
    let never = query(Some(NoteUsefulFilter::Never), None);
    let by_useful = query(None, Some(NoteSort::Useful));

    for live in [&ever, &never, &by_useful] {
        assert_eq!(
            cached(live.clone()).await.availability,
            MemoryScriptsAvailability::Available
        );
    }

    setup.upgrade(&[("FAKE_MEMORY_OFFLINE", "1")]);

    let kept_ever = cached(ever).await;
    let kept_never = cached(never).await;
    let kept_by_useful = cached(by_useful).await;

    assert!(kept_ever.offline && kept_never.offline && kept_by_useful.offline);
    assert!(kept_ever.cached_at.is_some());
    assert_eq!(ids(&kept_ever), [1, 2, 4, 7]);
    assert_eq!(ids(&kept_never), [3, 5]);
    assert_eq!(&ids(&kept_by_useful)[..4], [1, 7, 2, 4]);
}
