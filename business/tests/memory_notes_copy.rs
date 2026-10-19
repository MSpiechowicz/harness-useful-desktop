//! Copying the notes saved on this computer to the team database through `harness-memory push
//! --json`, against the stand-in of `fake_agent` (see `src/bin/fake_agent/memory_push.rs`): the
//! dry run and its counts, the rows paged and filtered from Rust in repository and title order, no
//! `--scope` ever named (the server picks the scopes routed to the team database alone), the picked
//! conflicts on stdin, what the copy did and the local ids left over, a failure redacted and never
//! left over, a note the dry run says will fail shown with its reason and never picked, a note
//! changed during the copy never left over, and an expired or older review refused.

use std::time::Duration;

use harness_business::BusinessError;
use harness_business::ecosystem::{NotesCopyCache, memory_config_path, preview_notes_copy};
use harness_business::model::{
    LocalNoteIds, MemoryCopyKind, NoteIdsWord, NotesCopyAction, NotesCopyCounts, NotesCopyFilter, NotesCopyPreview,
    NotesCopyResult,
};
use memory_notes_copy_support::{REVIEWED, args, assert_nothing_to_discard, assert_undeleted_is_left_over, ids};
use memory_stand_in::{CURRENT, Setup, Vars};
use pretty_assertions::assert_eq;

mod memory_notes_copy_support;
mod memory_stand_in;

const NOTES: MemoryCopyKind = MemoryCopyKind::Notes;
const NOTHING: Vars = &[("FAKE_MEMORY_PUSH", "nothing")];
const OLDER: Vars = &[("FAKE_MEMORY_PUSH", "usage")];
const UNFLAGGED: Vars = &[("FAKE_MEMORY_PUSH", "unflagged")];
const RELATION: Vars = &[("FAKE_MEMORY_PUSH", "relation")];
const UNDELETED: Vars = &[("FAKE_MEMORY_PUSH", "undeleted")];
const HIDDEN: Vars = &[("FAKE_MEMORY_PUSH", "hidden")];
const CHANGED: Vars = &[("FAKE_MEMORY_PUSH", "changed")];
/// Both note scopes kept here too, by the server's environment (which this app does not read).
const KEPT_HERE: Vars = &[
    ("HARNESS_MEMORY_REPOSITORY_NOTES", "remote+local"),
    ("HARNESS_MEMORY_GLOBAL_NOTES", "remote+local"),
];

async fn preview(setup: &Setup, cache: &NotesCopyCache) -> NotesCopyPreview {
    memory_notes_copy_support::preview(setup, cache, NOTES).await
}

async fn apply(
    setup: &Setup,
    cache: &NotesCopyCache,
    id: &str,
    overwrite: LocalNoteIds,
) -> Result<NotesCopyResult, BusinessError> {
    memory_notes_copy_support::apply(setup, cache, NOTES, id, overwrite).await
}

async fn copy_without_overwriting(vars: Vars) -> (Setup, NotesCopyResult) {
    memory_notes_copy_support::copy_without_overwriting(NOTES, vars).await
}

#[tokio::test]
async fn a_dry_run_counts_every_local_note_and_pages_them_by_repository_and_title() {
    let setup = Setup::installed(CURRENT, &[]);
    let cache = NotesCopyCache::new();

    let review = preview(&setup, &cache).await;
    assert_eq!(review.nothing, None);
    assert_eq!(review.total, 7);
    assert_eq!(review.counts, REVIEWED);
    assert_eq!(
        ids(&review.rows),
        [6, 3, 2, 7, 4, 5, 1],
        "global first, then by repository and title"
    );
    assert_eq!(review.rows[1].action, NotesCopyAction::Keep);
    assert!(review.rows[1].conflict);
    assert_eq!(review.rows[2].team_updated_at.as_deref(), Some("2026-10-09T10:00:00Z"));
    assert_eq!(review.rows[3].repo.as_deref(), Some("github.com/a/b"));
    assert!(
        review.rows.iter().all(|row| !row.failed && row.failure.is_none()),
        "nothing fails"
    );

    assert_eq!(args(&setup, 0), ["push", "--dry-run", "--json", "--overwrite", "none"]);
    assert_eq!(setup.runs()[0].timeout, Duration::from_secs(120));

    let page = cache
        .rows(NOTES, &review.preview_id, NotesCopyFilter::All, Some(2), Some(3))
        .expect("a page");
    assert_eq!((page.total, ids(&page.rows)), (7, vec![2, 7, 4]));

    let conflicts = cache
        .rows(NOTES, &review.preview_id, NotesCopyFilter::Conflicts, None, None)
        .expect("the conflicts");
    assert_eq!((conflicts.total, ids(&conflicts.rows)), (2, vec![3, 4]));

    for refused in [
        cache.rows(NOTES, "another review", NotesCopyFilter::All, None, None),
        cache.rows(NOTES, &review.preview_id, NotesCopyFilter::All, None, Some(0)),
        cache.rows(NOTES, &review.preview_id, NotesCopyFilter::All, None, Some(201)),
    ] {
        assert!(matches!(refused, Err(BusinessError::InvalidInput(_))), "{refused:?}");
    }

    let wire = serde_json::to_value(&review).unwrap();
    assert_eq!(wire["counts"]["teamNewer"], 1);
    assert_eq!(wire["counts"]["failed"], 0);
    assert_eq!(wire["rows"][0]["failed"], false);
    assert_eq!(wire["rows"][0]["failure"], serde_json::Value::Null);
    assert_eq!(wire["rows"][4]["action"], "keep");
    assert_eq!(wire["rows"][5]["action"], "teamNewer");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn the_copy_overwrites_the_picked_conflicts_and_reports_what_stays() {
    let setup = Setup::installed(CURRENT, &[]);
    let cache = NotesCopyCache::new();
    let review = preview(&setup, &cache).await;

    let picked = LocalNoteIds::Ids(vec![4]);
    let copied = apply(&setup, &cache, &review.preview_id, picked).await.expect("copied");

    assert_eq!(
        args(&setup, 1),
        ["push", "--json", "--overwrite", "-", "--delete-copied"]
    );
    assert_eq!(setup.runs()[1].stdin.as_deref(), Some(b"4\n".as_slice()));
    assert_eq!(setup.runs()[1].timeout, Duration::from_secs(300));

    assert_eq!(
        copied.counts,
        NotesCopyCounts {
            added: 1,
            updated: 1,
            overwritten: 1,
            identical: 1,
            team_newer: 1,
            conflicts: 2,
            failed: 1,
        },
        "the failed note is counted as failed alone"
    );
    assert_eq!(
        copied.deleted, 4,
        "added (one failed), updated, overwritten and identical"
    );
    assert_eq!(
        copied.kept, 2,
        "the conflict not picked and the team-newer note, never the failed one"
    );
    assert_eq!(
        copied.leftover,
        [3, 5],
        "the conflict not picked and the team-newer note, never the failed one"
    );
    assert_eq!(
        serde_json::to_value(&copied).unwrap()["leftover"],
        serde_json::json!([3, 5])
    );

    let [failure] = copied.failed.as_slice() else {
        panic!("one failure: {:?}", copied.failed);
    };
    assert_eq!(failure.local_id, Some(7));
    assert_eq!(failure.title.as_deref(), Some("A note the team refuses"));
    assert!(!failure.message.contains("hunter2"), "{}", failure.message);

    let again = apply(&setup, &cache, &review.preview_id, LocalNoteIds::Word(NoteIdsWord::All)).await;
    assert!(
        matches!(again, Err(BusinessError::InvalidInput(_))),
        "used up: {again:?}"
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn all_overwrites_exactly_the_conflicts_of_the_review() {
    let setup = Setup::installed(CURRENT, &[]);
    let cache = NotesCopyCache::new();
    let review = preview(&setup, &cache).await;

    let copied = apply(&setup, &cache, &review.preview_id, LocalNoteIds::Word(NoteIdsWord::All))
        .await
        .expect("copied");

    assert_eq!(
        args(&setup, 1),
        ["push", "--json", "--overwrite", "-", "--delete-copied"],
        "never `--overwrite all`, which would take conflicts that appeared since"
    );
    assert_eq!(setup.runs()[1].stdin.as_deref(), Some(b"3,4\n".as_slice()));
    assert_eq!((copied.counts.overwritten, copied.leftover), (2, vec![5]));

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_pick_that_is_not_a_conflict_is_refused_and_the_review_stays() {
    let setup = Setup::installed(CURRENT, &[]);
    let cache = NotesCopyCache::new();
    let review = preview(&setup, &cache).await;

    let not_a_conflict = apply(&setup, &cache, &review.preview_id, LocalNoteIds::Ids(vec![3, 1])).await;
    assert!(
        matches!(not_a_conflict, Err(BusinessError::InvalidInput(_))),
        "{not_a_conflict:?}"
    );
    assert_eq!(setup.runs().len(), 1, "nothing copied");

    let none = apply(
        &setup,
        &cache,
        &review.preview_id,
        LocalNoteIds::Word(NoteIdsWord::None),
    )
    .await
    .expect("copied");
    assert_eq!(args(&setup, 1)[..4], ["push", "--json", "--overwrite", "none"]);
    assert_eq!((none.counts.overwritten, none.kept), (0, 3));
    assert_eq!(none.leftover, [3, 4, 5]);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn no_scope_is_ever_named_so_a_scope_kept_here_is_never_pushed() {
    let setup = Setup::installed(CURRENT, KEPT_HERE);
    let cache = NotesCopyCache::new();

    // The config file alone routes global notes to the team database only.
    if let Some(config) = memory_config_path(&setup.env) {
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(
            &config,
            "database_url = \"postgres://h/db\"\nrepository_notes = \"remote+local\"\nglobal_notes = \"remote\"\n",
        )
        .unwrap();
    }

    let review = preview(&setup, &cache).await;
    assert_eq!(args(&setup, 0), ["push", "--dry-run", "--json", "--overwrite", "none"]);
    assert_eq!(
        (review.total, review.rows.len()),
        (0, 0),
        "the server skips every scope its environment keeps here"
    );

    let copied = apply(&setup, &cache, &review.preview_id, LocalNoteIds::Word(NoteIdsWord::All))
        .await
        .expect("copied");
    assert_eq!(
        args(&setup, 1),
        ["push", "--json", "--overwrite", "none", "--delete-copied"],
        "all of no conflicts is none"
    );
    assert!(copied.leftover.is_empty(), "{:?}", copied.leftover);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn an_older_server_without_the_failed_flag_still_keeps_the_failed_note_out() {
    let (setup, copied) = copy_without_overwriting(UNFLAGGED).await;
    assert_eq!(copied.failed.len(), 1);
    assert_eq!(copied.counts.added, 1, "the failed note is not counted as copied");
    assert_eq!(copied.leftover, [3, 4, 5], "never the failed note 7");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_failure_that_is_no_one_notes_is_reported_and_leaves_nothing_to_discard() {
    let (setup, copied) = copy_without_overwriting(RELATION).await;

    assert_nothing_to_discard(&copied, 7);

    let wire = serde_json::to_value(&copied).unwrap();
    assert_eq!(wire["failed"][1]["localId"], serde_json::Value::Null);
    assert_eq!(wire["leftover"], serde_json::json!([]));

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_note_copied_whose_local_copy_was_not_deleted_is_offered_for_discarding() {
    let (setup, copied) = copy_without_overwriting(UNDELETED).await;

    assert_undeleted_is_left_over(&copied, 7, &[1, 3, 4, 5]);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_note_the_dry_run_says_will_fail_shows_why_and_is_never_picked() {
    let setup = Setup::installed(CURRENT, HIDDEN);
    let cache = NotesCopyCache::new();

    let review = preview(&setup, &cache).await;
    assert_eq!(review.total, 7, "the failing note is still listed");
    assert_eq!(
        review.counts,
        NotesCopyCounts {
            conflicts: 1,
            failed: 1,
            ..REVIEWED
        },
        "the failing conflict counts as failed alone"
    );

    let failing = review.rows.iter().find(|row| row.local_id == 4).expect("note 4");
    let reason = failing.failure.as_deref().expect("why it will fail");
    assert!(failing.failed);
    assert!(reason.contains("unhide 34"), "{reason}");
    assert!(!reason.contains("hunter2"), "{reason}");
    assert_eq!(review.rows.iter().filter(|row| row.failed).count(), 1);

    let failed = cache
        .rows(NOTES, &review.preview_id, NotesCopyFilter::Failed, None, None)
        .expect("the failed rows");
    assert_eq!((failed.total, ids(&failed.rows)), (1, vec![4]));

    let conflicts = cache
        .rows(NOTES, &review.preview_id, NotesCopyFilter::Conflicts, None, None)
        .expect("the conflicts");
    assert_eq!((conflicts.total, ids(&conflicts.rows)), (1, vec![3]));

    let picked = apply(&setup, &cache, &review.preview_id, LocalNoteIds::Ids(vec![4])).await;
    assert!(
        matches!(&picked, Err(BusinessError::InvalidInput(message)) if message.contains("remote one")),
        "{picked:?}"
    );
    assert_eq!(setup.runs().len(), 1, "nothing copied");

    let copied = apply(&setup, &cache, &review.preview_id, LocalNoteIds::Word(NoteIdsWord::All))
        .await
        .expect("copied");
    assert_eq!(
        setup.runs()[1].stdin.as_deref(),
        Some(b"3\n".as_slice()),
        "all is the conflicts that do not fail"
    );
    assert_eq!((copied.counts.failed, copied.failed.len()), (2, 2));
    assert_eq!(
        (copied.deleted, copied.kept),
        (4, 1),
        "the failed notes are neither deleted nor kept"
    );
    assert_eq!(copied.leftover, [5]);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_note_changed_during_the_copy_is_kept_but_never_offered_for_discarding() {
    let (setup, copied) = copy_without_overwriting(CHANGED).await;

    assert_eq!(copied.counts.identical, 1, "note 2 was copied");
    assert_eq!(
        (copied.deleted, copied.kept),
        (2, 4),
        "note 2 holds newer edits, so it stays"
    );
    assert_eq!(copied.leftover, [3, 4, 5], "never note 2, nor the failed note 7");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn nothing_to_copy_and_an_older_server() {
    let setup = Setup::installed(CURRENT, NOTHING);
    let review = preview(&setup, &NotesCopyCache::new()).await;
    assert_eq!(review.nothing.as_deref(), Some("no notes on this computer"));
    assert_eq!((review.total, review.rows.len()), (0, 0));
    setup.scripts.shutdown().await;

    let older = Setup::installed(CURRENT, OLDER);
    let refused = preview_notes_copy(
        &older.env,
        &older.scripts,
        older.runner(),
        &NotesCopyCache::new(),
        NOTES,
    )
    .await;
    assert!(matches!(refused, Err(BusinessError::Unsupported(_))), "{refused:?}");
    older.scripts.shutdown().await;

    let missing = Setup::without_server(CURRENT, &[]);
    let refused = preview_notes_copy(
        &missing.env,
        &missing.scripts,
        missing.runner(),
        &NotesCopyCache::new(),
        NOTES,
    )
    .await;
    assert!(matches!(refused, Err(BusinessError::NotFound(_))), "{refused:?}");
    assert!(missing.requested().is_empty());
}
