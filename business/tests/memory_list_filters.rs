//! The combined list filters of `harness-memory notes list` (`--kind` with `--scope`) and
//! `scripts list` (`--scope` with `--review`, never with `--group`) against the stand-in of
//! `fake_agent` (see `src/bin/fake_agent/memory_cli.rs`): the flags only when asked, the counts of
//! each scope and review read, and a server that predates them asked once more without them, its
//! page marked as not combining filters.

use std::ffi::OsString;

use harness_business::BusinessError;
use harness_business::ecosystem::{ListWindow, NotesQuery, ScriptsQuery, list_notes};
use harness_business::model::{
    MemoryNotesPage, MemoryScopeCounts, MemoryScriptsAvailability, MemoryScriptsPage, NoteKind, NoteKindCount,
    ScriptGroup, ScriptReview, ScriptReviewCounts, ScriptScope, ScriptSort,
};
use memory_stand_in::{CURRENT, SINGLE, Setup, UNPAGED};
use pretty_assertions::assert_eq;

mod memory_stand_in;

fn window() -> ListWindow {
    ListWindow::new(None, None, None, None).unwrap()
}

fn notes_query(kind: Option<NoteKind>, scope: Option<ScriptScope>) -> NotesQuery {
    NotesQuery {
        window: window(),
        kind,
        scope,
        sort: None,
        hidden: false,
    }
}

fn scripts_query(scope: Option<ScriptScope>, review: Option<ScriptReview>) -> ScriptsQuery {
    ScriptsQuery::new(window(), None, scope, review, Some(ScriptSort::Name)).unwrap()
}

async fn notes(setup: &Setup, query: &NotesQuery) -> MemoryNotesPage {
    let project = setup.project();

    list_notes(&setup.env, &setup.scripts, setup.runner(), Some(&project), query).await
}

async fn scripts(setup: &Setup, query: &ScriptsQuery) -> MemoryScriptsPage {
    let project = setup.project();

    setup
        .scripts
        .list(&setup.env, setup.runner(), Some(&project), query)
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
