//! The combined list filters of `harness-memory notes list` (`--kind` with `--scope`) and
//! `scripts list` (`--scope` with `--review`, never with `--group`), and the place of both
//! (`--place`), against the stand-in of `fake_agent` (see `src/bin/fake_agent/memory_cli.rs`): the
//! flags only when asked, the counts of each scope, review and place read, and a server that
//! predates them asked again without them, its page marked as not combining filters or without
//! place counts.

use std::ffi::OsString;

use harness_business::BusinessError;
use harness_business::ecosystem::{ListWindow, NotesQuery, Repos, ScriptsQuery, list_notes};
use harness_business::model::{
    MemoryNotesPage, MemoryPlaceCounts, MemoryScopeCounts, MemoryScriptsAvailability, MemoryScriptsPage, NoteKind,
    NoteKindCount, ScriptGroup, ScriptPlace, ScriptReview, ScriptReviewCounts, ScriptScope, ScriptSort,
};
use memory_stand_in::{CURRENT, SCOPED, SINGLE, Setup, UNPAGED, Vars};
use pretty_assertions::assert_eq;

use crate::memory_stand_in;

fn window() -> ListWindow {
    ListWindow::new(None, None, None, None).unwrap()
}

fn notes_query(kind: Option<NoteKind>, scope: Option<ScriptScope>) -> NotesQuery {
    NotesQuery {
        window: window(),
        kind,
        scope,
        place: None,
        sort: None,
        hidden: false,
    }
}

fn scripts_query(scope: Option<ScriptScope>, review: Option<ScriptReview>) -> ScriptsQuery {
    ScriptsQuery::new(window(), None, scope, review, Some(ScriptSort::Name)).unwrap()
}

async fn notes(setup: &Setup, query: &NotesQuery) -> MemoryNotesPage {
    let project = setup.project();

    list_notes(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        Repos::Project(Some(&project)),
        query,
    )
    .await
}

async fn scripts(setup: &Setup, query: &ScriptsQuery) -> MemoryScriptsPage {
    let project = setup.project();

    setup
        .scripts
        .list(&setup.env, setup.runner(), Repos::Project(Some(&project)), query)
        .await
}

/// The arguments of run `index` after `list --json --offset 0 --limit 50`, up to `--sort`.
fn filters(setup: &Setup, index: usize) -> Vec<String> {
    let args: Vec<String> = setup.runs()[index]
        .args
        .iter()
        .map(|arg: &OsString| arg.to_string_lossy().into_owned())
        .collect();
    let sort = args.iter().position(|arg| arg == "--sort").unwrap();

    args[7..sort].to_vec()
}

#[tokio::test]
async fn notes_combine_kind_and_scope_and_count_each_scope() {
    let setup = Setup::installed(CURRENT, &[]);

    let all = notes(&setup, &notes_query(None, None)).await;
    assert!(
        all.combined_filters,
        "a server that answers the scope counts combines filters"
    );
    assert_eq!(all.scopes, Some(MemoryScopeCounts { repo: 3, global: 5 }));
    assert!(filters(&setup, 0).is_empty(), "no filter flags unless asked");

    let repo = notes(&setup, &notes_query(None, Some(ScriptScope::Repo))).await;
    assert_eq!(repo.notes.iter().map(|note| note.id).collect::<Vec<_>>(), [2, 3, 5]);
    assert_eq!(repo.total, 3);
    assert_eq!(
        repo.kinds.iter().map(|count| count.kind).collect::<Vec<_>>(),
        [NoteKind::Fix, NoteKind::Problem, NoteKind::Optimisation],
        "the kinds under the scope"
    );
    assert_eq!(
        repo.scopes,
        Some(MemoryScopeCounts { repo: 3, global: 5 }),
        "both scopes, whatever scope was asked for"
    );
    assert_eq!(filters(&setup, 1), ["--scope", "repo"]);

    let global_fixes = notes(&setup, &notes_query(Some(NoteKind::Fix), Some(ScriptScope::Global))).await;
    assert!(global_fixes.notes.is_empty() && global_fixes.total == 0);
    assert_eq!(
        global_fixes.scopes,
        Some(MemoryScopeCounts { repo: 1, global: 0 }),
        "under the kind"
    );
    assert!(global_fixes.kinds.contains(&NoteKindCount {
        kind: NoteKind::Decision,
        count: 1
    }));
    assert_eq!(filters(&setup, 2), ["--kind", "fix", "--scope", "global"]);
    assert_eq!(setup.runs().len(), 3, "one run per page");

    let wire = serde_json::to_value(&repo).unwrap();
    assert_eq!(wire["combinedFilters"], true);
    assert_eq!(wire["scopes"], serde_json::json!({ "repo": 3, "global": 5 }));

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn an_older_server_lists_notes_again_without_the_scope() {
    let setup = Setup::installed(CURRENT, SINGLE);

    let unfiltered = notes(&setup, &notes_query(Some(NoteKind::Fix), None)).await;
    assert_eq!(unfiltered.availability, MemoryScriptsAvailability::Available);
    assert!(!unfiltered.combined_filters, "no scope counts");
    assert_eq!(unfiltered.scopes, None);
    assert_eq!(setup.runs().len(), 1, "nothing to drop");

    let page = notes(&setup, &notes_query(Some(NoteKind::Fix), Some(ScriptScope::Global))).await;
    assert_eq!(page.availability, MemoryScriptsAvailability::Available);
    assert!(!page.combined_filters);
    assert_eq!(page.scopes, None);
    assert_eq!(
        page.notes.iter().map(|note| note.id).collect::<Vec<_>>(),
        [2],
        "the kind only"
    );

    assert_eq!(setup.runs().len(), 3, "asked once more");
    assert_eq!(filters(&setup, 1), ["--kind", "fix", "--scope", "global"]);
    assert_eq!(filters(&setup, 2), ["--kind", "fix"]);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn scripts_combine_scope_and_review_and_count_both() {
    let setup = Setup::installed(CURRENT, &[]);

    let global_needed = scripts(
        &setup,
        &scripts_query(Some(ScriptScope::Global), Some(ScriptReview::Needed)),
    )
    .await;
    assert_eq!(global_needed.availability, MemoryScriptsAvailability::Available);
    assert!(global_needed.combined_filters);
    assert_eq!(
        global_needed.scripts.iter().map(|script| script.id).collect::<Vec<_>>(),
        [7]
    );
    assert_eq!(global_needed.total, 1);
    assert_eq!(
        global_needed.scopes,
        Some(MemoryScopeCounts { repo: 0, global: 1 }),
        "under the review"
    );
    assert_eq!(
        global_needed.review,
        Some(ScriptReviewCounts { needed: 1, trusted: 0 }),
        "under the scope"
    );
    assert_eq!(filters(&setup, 0), ["--scope", "global", "--review", "needed"]);

    let trusted = scripts(&setup, &scripts_query(None, Some(ScriptReview::Trusted))).await;
    assert_eq!(
        trusted.scripts.iter().map(|script| script.id).collect::<Vec<_>>(),
        [12],
        "the entry of an unknown scope is skipped"
    );
    assert_eq!(trusted.total, 2, "the server's count");
    assert_eq!(trusted.review, Some(ScriptReviewCounts { needed: 1, trusted: 2 }));
    assert_eq!(filters(&setup, 1), ["--review", "trusted"]);

    let wire = serde_json::to_value(&trusted).unwrap();
    assert_eq!(wire["review"], serde_json::json!({ "needed": 1, "trusted": 2 }));
    assert_eq!(wire["combinedFilters"], true);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn an_older_server_lists_scripts_again_without_scope_and_review() {
    let setup = Setup::installed(CURRENT, SINGLE);

    let page = scripts(
        &setup,
        &scripts_query(Some(ScriptScope::Global), Some(ScriptReview::Needed)),
    )
    .await;
    assert_eq!(page.availability, MemoryScriptsAvailability::Available);
    assert!(!page.combined_filters);
    assert_eq!((page.scopes, page.review), (None, None));
    assert_eq!(
        page.scripts.iter().map(|script| script.id).collect::<Vec<_>>(),
        [12, 7],
        "every script"
    );
    assert_eq!(page.total, 3);

    assert_eq!(setup.runs().len(), 2);
    assert_eq!(filters(&setup, 0), ["--scope", "global", "--review", "needed"]);
    assert!(filters(&setup, 1).is_empty());

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_server_that_refuses_the_list_again_is_still_outdated() {
    let setup = Setup::installed(CURRENT, UNPAGED);

    let notes = notes(&setup, &notes_query(None, Some(ScriptScope::Repo))).await;
    assert_eq!(notes.availability, MemoryScriptsAvailability::Outdated);
    assert!(!notes.combined_filters);

    let scripts = scripts(&setup, &scripts_query(Some(ScriptScope::Repo), None)).await;
    assert_eq!(scripts.availability, MemoryScriptsAvailability::Outdated);
    assert!(scripts.scripts.is_empty() && scripts.scopes.is_none());

    setup.scripts.shutdown().await;
}

/// A team database for scripts, so that some are kept there.
const TEAM_SCRIPTS: Vars = &[("HARNESS_MEMORY_SCRIPTS", "remote")];

/// [`TEAM_SCRIPTS`] on a server that predates the place.
const SCOPED_TEAM_SCRIPTS: Vars = &[("FAKE_MEMORY_CLI", "scoped"), ("HARNESS_MEMORY_SCRIPTS", "remote")];

fn placed_notes(kind: Option<NoteKind>, scope: Option<ScriptScope>, place: ScriptPlace) -> NotesQuery {
    NotesQuery {
        place: Some(place),
        ..notes_query(kind, scope)
    }
}

fn placed_scripts(scope: Option<ScriptScope>, place: ScriptPlace) -> ScriptsQuery {
    ScriptsQuery {
        place: Some(place),
        ..scripts_query(scope, None)
    }
}

fn note_ids(page: &MemoryNotesPage) -> Vec<i64> {
    page.notes.iter().map(|note| note.id).collect()
}

fn script_ids(page: &MemoryScriptsPage) -> Vec<i64> {
    page.scripts.iter().map(|script| script.id).collect()
}

#[tokio::test]
async fn notes_filter_by_place_and_count_each_place() {
    let setup = Setup::installed(CURRENT, &[]);

    let all = notes(&setup, &notes_query(None, None)).await;
    assert_eq!(all.places, Some(MemoryPlaceCounts { local: 5, team: 3 }));
    assert!(filters(&setup, 0).is_empty(), "no place unless asked");

    let local = notes(&setup, &placed_notes(None, None, ScriptPlace::Local)).await;
    assert_eq!(note_ids(&local), [3, 5, 6, 7], "the note of an unknown kind is skipped");
    assert_eq!(local.total, 5, "the server's count");
    assert_eq!(
        local.places,
        Some(MemoryPlaceCounts { local: 5, team: 3 }),
        "both places, whatever place was asked for"
    );
    assert_eq!(
        local.scopes,
        Some(MemoryScopeCounts { repo: 2, global: 3 }),
        "under the place"
    );
    assert_eq!(filters(&setup, 1), ["--place", "local"]);

    let team_fixes = notes(
        &setup,
        &placed_notes(Some(NoteKind::Fix), Some(ScriptScope::Repo), ScriptPlace::Team),
    )
    .await;
    assert_eq!(note_ids(&team_fixes), [2]);
    assert_eq!(
        team_fixes.places,
        Some(MemoryPlaceCounts { local: 0, team: 1 }),
        "under the kind and scope"
    );
    assert_eq!(
        filters(&setup, 2),
        ["--kind", "fix", "--scope", "repo", "--place", "team"]
    );
    assert_eq!(setup.runs().len(), 3, "one run per page");

    let wire = serde_json::to_value(&team_fixes).unwrap();
    assert_eq!(wire["places"], serde_json::json!({ "local": 0, "team": 1 }));

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_server_without_the_place_lists_notes_again_with_the_other_filters() {
    let setup = Setup::installed(CURRENT, SCOPED);

    let page = notes(&setup, &placed_notes(None, Some(ScriptScope::Repo), ScriptPlace::Team)).await;
    assert_eq!(page.availability, MemoryScriptsAvailability::Available);
    assert_eq!(page.places, None, "no place counts: the place was dropped");
    assert!(page.combined_filters, "the scope still holds");
    assert_eq!(note_ids(&page), [2, 3, 5], "every place of the scope");

    assert_eq!(setup.runs().len(), 2, "asked once more");
    assert_eq!(filters(&setup, 0), ["--scope", "repo", "--place", "team"]);
    assert_eq!(filters(&setup, 1), ["--scope", "repo"]);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_server_without_combined_filters_drops_the_place_then_the_scope() {
    let setup = Setup::installed(CURRENT, SINGLE);

    let page = notes(
        &setup,
        &placed_notes(Some(NoteKind::Fix), Some(ScriptScope::Global), ScriptPlace::Local),
    )
    .await;
    assert_eq!(page.availability, MemoryScriptsAvailability::Available);
    assert_eq!((page.places, page.scopes), (None, None));
    assert!(!page.combined_filters);
    assert_eq!(note_ids(&page), [2], "the kind only");

    let place_only = notes(&setup, &placed_notes(None, None, ScriptPlace::Team)).await;
    assert_eq!(place_only.places, None);
    assert_eq!(place_only.total, 8, "every note");

    let runs: Vec<Vec<String>> = (0..setup.runs().len()).map(|index| filters(&setup, index)).collect();
    assert_eq!(
        runs,
        [
            vec!["--kind", "fix", "--scope", "global", "--place", "local"],
            vec!["--kind", "fix", "--scope", "global"],
            vec!["--kind", "fix"],
            vec!["--place", "team"],
            vec![],
        ]
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn scripts_filter_by_place_and_count_each_place() {
    let setup = Setup::installed(CURRENT, TEAM_SCRIPTS);

    let team = scripts(&setup, &placed_scripts(None, ScriptPlace::Team)).await;
    assert_eq!(script_ids(&team), [31, 30], "by name");
    assert_eq!(team.total, 2);
    assert_eq!(team.places, Some(MemoryPlaceCounts { local: 3, team: 2 }));
    assert_eq!(filters(&setup, 0), ["--place", "team"]);

    let local_repo = scripts(&setup, &placed_scripts(Some(ScriptScope::Repo), ScriptPlace::Local)).await;
    assert_eq!(script_ids(&local_repo), [12]);
    assert_eq!(
        local_repo.places,
        Some(MemoryPlaceCounts { local: 1, team: 1 }),
        "under the scope"
    );
    assert_eq!(
        local_repo.scopes,
        Some(MemoryScopeCounts { repo: 1, global: 1 }),
        "under the place"
    );
    assert_eq!(filters(&setup, 1), ["--scope", "repo", "--place", "local"]);

    let wire = serde_json::to_value(&local_repo).unwrap();
    assert_eq!(wire["places"], serde_json::json!({ "local": 1, "team": 1 }));

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_server_without_the_place_lists_scripts_again_with_the_other_filters() {
    let setup = Setup::installed(CURRENT, SCOPED_TEAM_SCRIPTS);

    let page = scripts(&setup, &placed_scripts(Some(ScriptScope::Global), ScriptPlace::Team)).await;
    assert_eq!(page.availability, MemoryScriptsAvailability::Available);
    assert_eq!(page.places, None);
    assert!(page.combined_filters);
    assert_eq!(script_ids(&page), [31, 7], "every place of the scope");

    assert_eq!(setup.runs().len(), 2);
    assert_eq!(filters(&setup, 0), ["--scope", "global", "--place", "team"]);
    assert_eq!(filters(&setup, 1), ["--scope", "global"]);

    setup.scripts.shutdown().await;
}

#[test]
fn a_script_group_does_not_combine_with_scope_or_review() {
    let with_group = |scope, review| ScriptsQuery::new(window(), Some(ScriptGroup::NeedsReview), scope, review, None);

    for result in [
        with_group(Some(ScriptScope::Repo), None),
        with_group(None, Some(ScriptReview::Trusted)),
    ] {
        assert!(matches!(result, Err(BusinessError::InvalidInput(_))), "{result:?}");
    }

    assert!(with_group(None, None).is_ok());
    assert!(
        ScriptsQuery::new(
            window(),
            None,
            Some(ScriptScope::Repo),
            Some(ScriptReview::Needed),
            None
        )
        .is_ok()
    );
}
