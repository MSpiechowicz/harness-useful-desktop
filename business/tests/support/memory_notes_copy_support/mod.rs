//! The review and copy calls of the tests of copying notes and scripts to the team database
//! (`memory_notes_copy.rs`, `memory_scripts_copy.rs`), against the stand-in of `fake_agent`.
#![allow(dead_code)]

use harness_business::BusinessError;
use harness_business::ecosystem::{
    CopyGate, CopyPicks, CopyTarget, MemoryFeatures, NotesCopyCache, apply_notes_copy, preview_notes_copy,
};
use harness_business::model::{
    LocalNoteIds, MemoryCopyKind, NoteIdsWord, NotesCopyCounts, NotesCopyMode, NotesCopyPreview, NotesCopyResult,
    NotesCopyRow,
};

use crate::memory_stand_in::{CURRENT, Setup, Vars};

/// The counts of a review of the stand-in's local notes, or of its local scripts (the same).
pub const REVIEWED: NotesCopyCounts = NotesCopyCounts {
    added: 2,
    updated: 1,
    overwritten: 0,
    identical: 1,
    team_newer: 1,
    local_differs: 0,
    conflicts: 2,
    failed: 0,
};

/// The switches of Settings that allow a copy to the team database and one to this computer.
#[derive(Debug, Clone, Copy)]
pub struct Switches {
    pub to_remote: bool,
    pub to_local: bool,
}

/// Both switches off, as they are by default: only `stranded` copies run.
pub const OFF: Switches = Switches {
    to_remote: false,
    to_local: false,
};

/// Both switches on.
pub const ON: Switches = Switches {
    to_remote: true,
    to_local: true,
};

/// A review of `target` with `switches`, against a server that lists `features` (its stand-in).
pub async fn try_preview_target(
    setup: &Setup,
    cache: &NotesCopyCache,
    target: CopyTarget,
    switches: Switches,
) -> Result<NotesCopyPreview, BusinessError> {
    let features = MemoryFeatures::new();
    let gate = CopyGate {
        features: &features,
        to_remote: switches.to_remote,
        to_local: switches.to_local,
    };

    preview_notes_copy(&setup.env, &setup.scripts, setup.runner(), &gate, cache, target).await
}

/// A copy of `target` with `switches` of the review `picks` names.
pub async fn try_apply_target(
    setup: &Setup,
    cache: &NotesCopyCache,
    target: CopyTarget,
    switches: Switches,
    picks: &CopyPicks<'_>,
) -> Result<NotesCopyResult, BusinessError> {
    let features = MemoryFeatures::new();
    let gate = CopyGate {
        features: &features,
        to_remote: switches.to_remote,
        to_local: switches.to_local,
    };

    apply_notes_copy(&setup.env, &setup.scripts, setup.runner(), &gate, cache, target, picks).await
}

pub const NOTES: MemoryCopyKind = MemoryCopyKind::Notes;
pub const SCRIPTS: MemoryCopyKind = MemoryCopyKind::Scripts;
pub const TO_REMOTE: NotesCopyMode = NotesCopyMode::ToRemote;
pub const TO_LOCAL: NotesCopyMode = NotesCopyMode::ToLocal;

/// Both note scopes and the scripts kept here too, by the server's environment (which this app does not read).
pub const KEPT_HERE: Vars = &[
    ("HARNESS_MEMORY_REPOSITORY_NOTES", "remote+local"),
    ("HARNESS_MEMORY_GLOBAL_NOTES", "remote+local"),
    ("HARNESS_MEMORY_SCRIPTS", "remote+local"),
];

/// A review of `target` with both switches on, and the cache that keeps it.
pub async fn reviewed(setup: &Setup, target: CopyTarget) -> (NotesCopyCache, NotesCopyPreview) {
    let cache = NotesCopyCache::new();
    let review = try_preview_target(setup, &cache, target, ON).await.expect("reviewed");

    (cache, review)
}

/// The `target` of `kind` going `mode`.
pub fn target(kind: MemoryCopyKind, mode: NotesCopyMode) -> CopyTarget {
    CopyTarget { kind, mode }
}

/// A `stranded` review of `kind` with both switches off, which must still run.
pub async fn try_preview(
    setup: &Setup,
    cache: &NotesCopyCache,
    kind: MemoryCopyKind,
) -> Result<NotesCopyPreview, BusinessError> {
    try_preview_target(setup, cache, kind.into(), OFF).await
}

pub async fn preview(setup: &Setup, cache: &NotesCopyCache, kind: MemoryCopyKind) -> NotesCopyPreview {
    try_preview(setup, cache, kind).await.expect("reviewed")
}

pub async fn apply(
    setup: &Setup,
    cache: &NotesCopyCache,
    kind: MemoryCopyKind,
    id: &str,
    overwrite: LocalNoteIds,
) -> Result<NotesCopyResult, BusinessError> {
    let picks = CopyPicks::overwriting(id, &overwrite);

    try_apply_target(setup, cache, kind.into(), OFF, &picks).await
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
    rows.iter().filter_map(|row| row.local_id).collect()
}

/// The team ids of `rows`.
pub fn team_ids(rows: &[NotesCopyRow]) -> Vec<i64> {
    rows.iter().filter_map(|row| row.team_id).collect()
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

/// The arguments of the runs of the server that copy (everything but its `features` probe).
pub fn copies(setup: &Setup) -> Vec<Vec<String>> {
    (0..setup.runs().len())
        .map(|run| setup.run_args(run))
        .filter(|args| args != &["features"])
        .collect()
}

/// The stdin of the `n`th run that copies.
pub fn stdin_of(setup: &Setup, n: usize) -> Option<String> {
    let runs: Vec<_> = setup
        .runs()
        .into_iter()
        .filter(|run| run.args.first().is_some_and(|arg| arg != "features"))
        .collect();

    runs[n]
        .stdin
        .as_deref()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
}

pub fn is_invalid<T: std::fmt::Debug>(answer: &Result<T, BusinessError>) -> bool {
    matches!(answer, Err(BusinessError::InvalidInput(_)))
}
