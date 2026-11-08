//! What a copy to the remote database sends in `--selection -` and what comes back, against the
//! stand-in of `fake_agent` (see `src/bin/fake_agent/memory_push.rs`): each picked row named with
//! its id, title or name and update time exactly as the dry run printed them, a row without an
//! update time not pickable, an entry saved after the review left out, a failing row never named, and
//! a row the server refuses as changed since the review reported per entry while the others are
//! still copied. Which rows a copy takes is in `memory_copy_remote_picks`.

use harness_business::ecosystem::{CopyPicks, NotesCopyCache};
use harness_business::model::{LocalNoteIds, MemoryCopyKind, NoteIdsWord, NotesCopyFilter};
use memory_notes_copy_support::{
    NOTES, ON, SCRIPTS, TO_REMOTE, stdin_of, target, try_apply_target, try_preview_target,
};
use memory_stand_in::{CURRENT, Setup, Vars};
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::memory_notes_copy_support;
use crate::memory_stand_in;

/// The notes of the stand-in, by local id, and its scripts: what a review names in `only`.
pub const NOTE_ENTRIES: [(i64, &str); 7] = [
    (1, "Watcher misses renames"),
    (2, "Prefer bun over npm"),
    (3, "Memory config lives in the config dir"),
    (4, "Bump the cache key"),
    (5, "retry the watcher on rename"),
    (6, "Cache the tools probe"),
    (7, "A note the team refuses"),
];
pub const SCRIPT_ENTRIES: [(i64, &str); 7] = [
    (11, "open-prs"),
    (12, "tidy-branches"),
    (13, "bump-cache"),
    (14, "lint-all"),
    (15, "watch-renames"),
    (16, "probe-tools"),
    (17, "refused-script"),
];

/// A server whose first entry was saved again after the review (a real run sees another update
/// time, or another title or name), and one that prints no update time for it.
const SAVED_AGAIN: Vars = &[
    ("FAKE_MEMORY_PUSH", "stale"),
    ("HARNESS_MEMORY_REPOSITORY_NOTES", "remote+local"),
    ("HARNESS_MEMORY_GLOBAL_NOTES", "remote+local"),
    ("HARNESS_MEMORY_SCRIPTS", "remote+local"),
];
const RENAMED_AFTER: Vars = &[
    ("FAKE_MEMORY_PUSH", "renamed"),
    ("HARNESS_MEMORY_REPOSITORY_NOTES", "remote+local"),
    ("HARNESS_MEMORY_GLOBAL_NOTES", "remote+local"),
    ("HARNESS_MEMORY_SCRIPTS", "remote+local"),
];
const WITHOUT_UPDATE_TIME: Vars = &[
    ("FAKE_MEMORY_PUSH", "undated"),
    ("HARNESS_MEMORY_REPOSITORY_NOTES", "remote+local"),
    ("HARNESS_MEMORY_GLOBAL_NOTES", "remote+local"),
    ("HARNESS_MEMORY_SCRIPTS", "remote+local"),
];

/// The entries of `kind` in the order of their local ids.
fn entries_of(kind: MemoryCopyKind) -> [(i64, &'static str); 7] {
    match kind {
        MemoryCopyKind::Notes => NOTE_ENTRIES,
        MemoryCopyKind::Scripts => SCRIPT_ENTRIES,
    }
}

/// The entries of `kind` whose notes are numbered `numbers` (1 to 7) in the stand-in.
pub fn entries_numbered(kind: MemoryCopyKind, numbers: &[usize]) -> Vec<(i64, &'static str)> {
    let entries = entries_of(kind);

    numbers.iter().map(|number| entries[number - 1]).collect()
}

/// The selection on stdin for `entries` reviewed (each with the update time the stand-in prints for
/// its id, `2026-10-10T10:00:<id>.250000Z`) and the conflicts `overwrite`.
pub fn selection(entries: &[(i64, &str)], overwrite: &[i64]) -> String {
    let only: Vec<_> = entries
        .iter()
        .map(|(id, name)| json!({ "id": id, "name": name, "updatedAt": format!("2026-10-10T10:00:{id:02}.250000Z") }))
        .collect();

    json!({ "only": only, "overwrite": overwrite }).to_string()
}

/// A server whose dry run leaves out an entry that a plain push would still take, as one saved after
/// the review.
const SAVED_LATER: Vars = &[
    ("FAKE_MEMORY_PUSH", "late"),
    ("HARNESS_MEMORY_REPOSITORY_NOTES", "remote+local"),
    ("HARNESS_MEMORY_GLOBAL_NOTES", "remote+local"),
    ("HARNESS_MEMORY_SCRIPTS", "remote+local"),
];

#[tokio::test]
async fn a_copy_to_remote_leaves_out_what_was_saved_after_the_review() {
    for kind in [NOTES, SCRIPTS] {
        let setup = Setup::installed(CURRENT, SAVED_LATER);
        let cache = NotesCopyCache::new();
        let to_remote = target(kind, TO_REMOTE);

        let review = try_preview_target(&setup, &cache, to_remote, ON)
            .await
            .expect("reviewed");
        assert_eq!(review.total, 6, "{kind:?}: the sixth entry did not exist yet");

        let all = LocalNoteIds::Word(NoteIdsWord::All);
        let copied = try_apply_target(
            &setup,
            &cache,
            to_remote,
            ON,
            &CopyPicks::picking(&review.preview_id, &all),
        )
        .await
        .expect("copied");

        let reviewed = entries_numbered(kind, &[1, 3, 4, 7]);
        let overwritten = &reviewed[1..3].iter().map(|entry| entry.0).collect::<Vec<_>>();

        assert_eq!(
            stdin_of(&setup, 1).as_deref(),
            Some(selection(&reviewed, overwritten).as_str()),
            "{kind:?}: only what was reviewed"
        );
        assert_eq!(
            (copied.counts.updated, copied.deleted),
            (0, 3),
            "{kind:?}: the entry saved later was neither copied nor deleted"
        );
        setup.scripts.shutdown().await;
    }
}

#[tokio::test]
async fn a_copy_to_remote_names_no_failing_row_in_its_selection() {
    let setup = Setup::installed(
        CURRENT,
        &[
            ("FAKE_MEMORY_PUSH", "hidden"),
            ("HARNESS_MEMORY_REPOSITORY_NOTES", "remote+local"),
            ("HARNESS_MEMORY_GLOBAL_NOTES", "remote+local"),
        ],
    );
    let cache = NotesCopyCache::new();
    let to_remote = target(NOTES, TO_REMOTE);
    let review = try_preview_target(&setup, &cache, to_remote, ON)
        .await
        .expect("reviewed");
    assert_eq!(review.counts.failed, 1);

    let all = LocalNoteIds::Word(NoteIdsWord::All);
    let picks = CopyPicks::picking(&review.preview_id, &all);
    let _ = try_apply_target(&setup, &cache, to_remote, ON, &picks).await;

    assert_eq!(
        stdin_of(&setup, 1).as_deref(),
        Some(selection(&entries_numbered(NOTES, &[1, 3, 6, 7]), &[3]).as_str()),
        "the fourth entry fails in the review, so it is not pickable"
    );
    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_row_changed_since_the_review_is_reported_per_entry_and_the_others_are_still_copied() {
    for (vars, edited) in [(SAVED_AGAIN, false), (RENAMED_AFTER, true)] {
        for kind in [NOTES, SCRIPTS] {
            let setup = Setup::installed(CURRENT, vars);
            let cache = NotesCopyCache::new();
            let to_remote = target(kind, TO_REMOTE);
            let (first_id, first_name) = entries_of(kind)[0];

            let review = try_preview_target(&setup, &cache, to_remote, ON)
                .await
                .expect("reviewed");
            assert_eq!((review.total, review.counts.failed), (7, 0), "{kind:?}");

            let all = LocalNoteIds::Word(NoteIdsWord::All);
            let copied = try_apply_target(
                &setup,
                &cache,
                to_remote,
                ON,
                &CopyPicks::picking(&review.preview_id, &all),
            )
            .await
            .expect("a refused row is a partial result, not a failed copy");

            let reviewed = entries_numbered(kind, &[1, 3, 4, 6, 7]);
            let overwritten = [reviewed[1].0, reviewed[2].0];

            assert_eq!(
                stdin_of(&setup, 1).as_deref(),
                Some(selection(&reviewed, &overwritten).as_str()),
                "{kind:?}: the review is sent as it was"
            );

            let stale = copied
                .failed
                .iter()
                .find(|failure| failure.local_id == Some(first_id))
                .unwrap_or_else(|| panic!("{kind:?} {edited}: {:?}", copied.failed));
            let title = if edited {
                format!("{first_name} (edited)")
            } else {
                first_name.to_owned()
            };

            assert_eq!(stale.title.as_deref(), Some(title.as_str()), "{kind:?}");
            assert_eq!(
                copied.failed.len(),
                2,
                "{kind:?}: the entry the team refuses fails too: {:?}",
                copied.failed
            );
            assert_eq!(copied.counts.failed, 2, "{kind:?}");
            assert_eq!(
                copied.deleted, 3,
                "{kind:?}: the other entries were copied and their local copies deleted, the stale one not"
            );
            assert!(!copied.leftover.contains(&first_id), "{kind:?}: {:?}", copied.leftover);

            setup.scripts.shutdown().await;
        }
    }
}

#[tokio::test]
async fn nothing_is_sent_for_a_row_without_an_update_time() {
    for kind in [NOTES, SCRIPTS] {
        let setup = Setup::installed(CURRENT, WITHOUT_UPDATE_TIME);
        let cache = NotesCopyCache::new();
        let to_remote = target(kind, TO_REMOTE);
        let entries = entries_of(kind);

        let review = try_preview_target(&setup, &cache, to_remote, ON)
            .await
            .expect("reviewed");
        let rows = cache
            .rows(to_remote, &review.preview_id, NotesCopyFilter::All, None, None)
            .expect("its rows");
        assert_eq!(rows.total, 7, "{kind:?}: the row is still shown");
        assert!(
            rows.rows
                .iter()
                .any(|row| row.local_id == Some(entries[0].0) && row.local_updated_at.is_none() && !row.pickable),
            "{kind:?}: not pickable, as it cannot be named"
        );

        let all = LocalNoteIds::Word(NoteIdsWord::All);
        let copied = try_apply_target(
            &setup,
            &cache,
            to_remote,
            ON,
            &CopyPicks::picking(&review.preview_id, &all),
        )
        .await
        .expect("copied");

        let named = entries_numbered(kind, &[3, 4, 6, 7]);
        let overwritten = [named[0].0, named[1].0];

        assert_eq!(
            stdin_of(&setup, 1).as_deref(),
            Some(selection(&named, &overwritten).as_str()),
            "{kind:?}: the row is not named"
        );
        assert!(
            copied
                .failed
                .iter()
                .all(|failure| failure.local_id != Some(entries[0].0)),
            "{kind:?}: not refused, just not asked for"
        );
        assert_eq!(copied.counts.added, 0, "{kind:?}: the undated entry was not copied");
        assert_eq!(copied.deleted, 3, "{kind:?}: nor was it deleted");
        setup.scripts.shutdown().await;
    }
}
