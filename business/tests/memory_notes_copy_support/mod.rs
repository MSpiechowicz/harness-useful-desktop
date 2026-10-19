//! The review and copy calls of the tests of copying notes and scripts to the team database
//! (`memory_notes_copy.rs`, `memory_scripts_copy.rs`), against the stand-in of `fake_agent`.
#![allow(dead_code)]

use harness_business::BusinessError;
use harness_business::ecosystem::{NotesCopyCache, apply_notes_copy, preview_notes_copy};
use harness_business::model::{
    LocalNoteIds, MemoryCopyKind, NoteIdsWord, NotesCopyCounts, NotesCopyPreview, NotesCopyResult, NotesCopyRow,
};

use crate::memory_stand_in::{CURRENT, Setup, Vars};

/// The counts of a review of the stand-in's local notes, or of its local scripts (the same).
pub const REVIEWED: NotesCopyCounts = NotesCopyCounts {
    added: 2,
    updated: 1,
    overwritten: 0,
    identical: 1,
    team_newer: 1,
    conflicts: 2,
    failed: 0,
};

pub async fn preview(setup: &Setup, cache: &NotesCopyCache, kind: MemoryCopyKind) -> NotesCopyPreview {
    preview_notes_copy(&setup.env, &setup.scripts, setup.runner(), cache, kind)
        .await
        .expect("reviewed")
}

pub async fn apply(
    setup: &Setup,
    cache: &NotesCopyCache,
    kind: MemoryCopyKind,
    id: &str,
    overwrite: LocalNoteIds,
) -> Result<NotesCopyResult, BusinessError> {
    apply_notes_copy(&setup.env, &setup.scripts, setup.runner(), cache, kind, id, &overwrite).await
}

/// A review and a copy of `kind` that overwrites no conflict, against a server with `vars`.
pub async fn copy_without_overwriting(kind: MemoryCopyKind, vars: Vars) -> (Setup, NotesCopyResult) {
    let setup = Setup::installed(CURRENT, vars);
    let cache = NotesCopyCache::new();
    let review = preview(&setup, &cache, kind).await;
    let none = LocalNoteIds::Word(NoteIdsWord::None);
    let copied = apply(&setup, &cache, kind, &review.preview_id, none)
        .await
        .expect("copied");

    (setup, copied)
}

/// The arguments of the server's run `run`.
pub fn args(setup: &Setup, run: usize) -> Vec<String> {
    setup.runs()[run]
        .args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

pub fn ids(rows: &[NotesCopyRow]) -> Vec<i64> {
    rows.iter().map(|row| row.local_id).collect()
}

/// That `copied` reports the failure of `failing` and one that is no one entry's (redacted, without
/// a title), deleted nothing, and leaves nothing to discard.
pub fn assert_nothing_to_discard(copied: &NotesCopyResult, failing: i64) {
    let [entry, relation] = copied.failed.as_slice() else {
        panic!("two failures: {:?}", copied.failed);
    };

    assert_eq!(entry.local_id, Some(failing));
    assert_eq!((relation.local_id, relation.title.as_deref()), (None, None));
    assert!(!relation.message.contains("hunter2"), "{}", relation.message);
    assert_eq!(copied.deleted, 0, "nothing deleted");
    assert!(
        copied.leftover.is_empty(),
        "no entry may be offered for discarding: {:?}",
        copied.leftover
    );
}

/// That `copied` counts the entry copied but whose local copy could not be deleted (the first of
/// `leftover`) as copied and offers it for discarding with the rest of `leftover`, while `failing`
/// stays out of both.
pub fn assert_undeleted_is_left_over(copied: &NotesCopyResult, failing: i64, leftover: &[i64]) {
    let failed: Vec<Option<i64>> = copied.failed.iter().map(|failure| failure.local_id).collect();

    assert_eq!(failed, [Some(failing), Some(leftover[0])]);
    assert_eq!(
        copied.counts.added, 1,
        "the undeleted entry is copied, the failed one is not"
    );
    assert_eq!((copied.deleted, copied.kept), (2, 4), "the failed entry is not kept");
    assert_eq!(copied.leftover, leftover, "never the failed entry");
}
