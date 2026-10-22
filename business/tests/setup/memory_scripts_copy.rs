//! Copying the scripts saved on this computer to the team database through `harness-memory scripts
//! push --json`, against the stand-in of `fake_agent` (see `src/bin/fake_agent/memory_push.rs`):
//! the dry run read from the scripts answer (a `name` per row, a `scripts` counts object), no
//! `--scope` ever named, `all` sent as the review's conflicts on stdin, a failure that is no one
//! script's leaving nothing to discard, and a review of one kind kept apart from the other's.

use harness_business::BusinessError;
use harness_business::ecosystem::{NotesCopyCache, preview_notes_copy};
use harness_business::model::{LocalNoteIds, MemoryCopyKind, NoteIdsWord, NotesCopyFilter};
use memory_notes_copy_support::{
    REVIEWED, apply, args, assert_nothing_to_discard, assert_undeleted_is_left_over, copy_without_overwriting, ids,
    preview,
};
use memory_stand_in::{CURRENT, Setup, Vars};
use pretty_assertions::assert_eq;

use crate::memory_notes_copy_support;
use crate::memory_stand_in;

const SCRIPTS: MemoryCopyKind = MemoryCopyKind::Scripts;
const NOTES: MemoryCopyKind = MemoryCopyKind::Notes;
const OLDER: Vars = &[("FAKE_MEMORY_PUSH", "usage")];
const RELATION: Vars = &[("FAKE_MEMORY_PUSH", "relation")];
const UNDELETED: Vars = &[("FAKE_MEMORY_PUSH", "undeleted")];
/// Scripts kept here too, by the server's environment (which this app does not read).
const KEPT_HERE: Vars = &[("HARNESS_MEMORY_SCRIPTS", "remote+local")];

#[tokio::test]
async fn a_scripts_dry_run_runs_scripts_push_and_reads_the_scripts_answer() {
    let setup = Setup::installed(CURRENT, &[]);
    let cache = NotesCopyCache::new();

    let review = preview(&setup, &cache, SCRIPTS).await;
    assert_eq!(
        args(&setup, 0),
        ["scripts", "push", "--dry-run", "--json", "--overwrite", "none"]
    );
    assert_eq!(review.total, 7);
    assert_eq!(review.counts, REVIEWED);
    assert_eq!(
        ids(&review.rows),
        [13, 16, 12, 14, 17, 11, 15],
        "global first, then by repository and name"
    );
    assert_eq!(review.rows[0].title, "bump-cache", "a script's name");

    let conflicts = cache
        .rows(SCRIPTS, &review.preview_id, NotesCopyFilter::Conflicts, None, None)
        .expect("the conflicts");
    assert_eq!((conflicts.total, ids(&conflicts.rows)), (2, vec![13, 14]));

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn all_overwrites_exactly_the_reviewed_script_conflicts() {
    let setup = Setup::installed(CURRENT, &[]);
    let cache = NotesCopyCache::new();
    let review = preview(&setup, &cache, SCRIPTS).await;

    let copied = apply(
        &setup,
        &cache,
        SCRIPTS,
        &review.preview_id,
        LocalNoteIds::Word(NoteIdsWord::All),
    )
    .await
    .expect("copied");

    assert_eq!(
        args(&setup, 1),
        ["scripts", "push", "--json", "--overwrite", "-", "--delete-copied"],
        "never `--overwrite all` and never `--scope`"
    );
    assert_eq!(setup.runs()[1].stdin.as_deref(), Some(b"13,14\n".as_slice()));
    assert_eq!((copied.counts.overwritten, copied.deleted), (2, 5));
    assert_eq!(copied.leftover, [15], "the team-newer script, never the failed one");

    let [failure] = copied.failed.as_slice() else {
        panic!("one failure: {:?}", copied.failed);
    };
    assert_eq!(failure.local_id, Some(17));
    assert_eq!(failure.title.as_deref(), Some("refused-script"));
    assert!(!failure.message.contains("hunter2"), "{}", failure.message);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_review_of_one_kind_is_kept_apart_from_the_other() {
    let setup = Setup::installed(CURRENT, &[]);
    let cache = NotesCopyCache::new();
    let notes = preview(&setup, &cache, NOTES).await;
    let scripts = preview(&setup, &cache, SCRIPTS).await;

    let notes_page = cache
        .rows(NOTES, &notes.preview_id, NotesCopyFilter::Conflicts, None, None)
        .expect("the notes review stays");
    assert_eq!(ids(&notes_page.rows), [3, 4]);

    for crossed in [
        cache.rows(SCRIPTS, &notes.preview_id, NotesCopyFilter::All, None, None),
        cache.rows(NOTES, &scripts.preview_id, NotesCopyFilter::All, None, None),
    ] {
        assert!(matches!(crossed, Err(BusinessError::InvalidInput(_))), "{crossed:?}");
    }

    let none = || LocalNoteIds::Word(NoteIdsWord::None);
    let crossed = apply(&setup, &cache, SCRIPTS, &notes.preview_id, none()).await;
    assert!(matches!(crossed, Err(BusinessError::InvalidInput(_))), "{crossed:?}");
    assert_eq!(setup.runs().len(), 2, "nothing copied");

    apply(&setup, &cache, SCRIPTS, &scripts.preview_id, none())
        .await
        .expect("scripts copied");
    assert_eq!(args(&setup, 2)[..2], ["scripts", "push"]);

    apply(&setup, &cache, NOTES, &notes.preview_id, none())
        .await
        .expect("the notes review was not used up by the scripts copy");
    assert_eq!(args(&setup, 3)[0], "push");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_failure_that_is_no_one_scripts_leaves_nothing_to_discard() {
    let (setup, copied) = copy_without_overwriting(SCRIPTS, RELATION).await;

    assert_nothing_to_discard(&copied, 17);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_script_copied_whose_local_copy_was_not_deleted_is_offered_for_discarding() {
    let (setup, copied) = copy_without_overwriting(SCRIPTS, UNDELETED).await;

    assert_undeleted_is_left_over(&copied, 17, &[11, 13, 14, 15]);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn scripts_kept_here_are_never_pushed_and_an_older_server_is_unsupported() {
    let setup = Setup::installed(CURRENT, KEPT_HERE);
    let review = preview(&setup, &NotesCopyCache::new(), SCRIPTS).await;
    assert_eq!(
        args(&setup, 0),
        ["scripts", "push", "--dry-run", "--json", "--overwrite", "none"]
    );
    assert_eq!(review.total, 0, "the server skips scripts its environment keeps here");
    setup.scripts.shutdown().await;

    let older = Setup::installed(CURRENT, OLDER);
    let refused = preview_notes_copy(
        &older.env,
        &older.scripts,
        older.runner(),
        &NotesCopyCache::new(),
        SCRIPTS,
    )
    .await;
    assert!(matches!(refused, Err(BusinessError::Unsupported(_))), "{refused:?}");
    older.scripts.shutdown().await;
}
