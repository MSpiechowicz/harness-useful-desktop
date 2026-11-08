//! Copying notes and scripts either way under "Remote + this computer", against the stand-in of
//! `fake_agent` (see `src/bin/fake_agent/memory_push.rs` and `memory_pull.rs`): `toRemote` is
//! `push --remote-local` and `toLocal` is `pull`, each refused (`forbidden`) while its Settings
//! switch is off and (`unsupported`) while the server does not list its feature, a review of one
//! target never used for another, the picked team ids on stdin one per line, `all` meaning the
//! review's own rows, no delete or scope flag ever passed to a pull, and `stranded` untouched: no
//! switch, no `features` probe, no `--remote-local`.
//! A copy to the remote names its rows in `memory_copy_selection`.

use harness_business::BusinessError;
use harness_business::ecosystem::{CopyPicks, NotesCopyCache};
use harness_business::model::{LocalNoteIds, NoteIdsWord, NotesCopyFilter, NotesCopyMode};
use memory_notes_copy_support::{
    KEPT_HERE, NOTES, OFF, ON, REVIEWED, SCRIPTS, Switches, TO_LOCAL, TO_REMOTE, copies, is_invalid, stdin_of, target,
    try_apply_target, try_preview_target,
};
use memory_stand_in::{CURRENT, Setup, Vars};
use pretty_assertions::assert_eq;

use crate::memory_copy_selection::{NOTE_ENTRIES, SCRIPT_ENTRIES, selection};
use crate::memory_notes_copy_support;
use crate::memory_stand_in;

/// A server that lists `push-remote-local` and the pulls but not `push-selection`.
const WITHOUT_SELECTION: Vars = &[
    (
        "FAKE_MEMORY_FEATURES",
        "push-json,scripts-push-json,push-remote-local,pull-json,scripts-pull-json",
    ),
    ("HARNESS_MEMORY_REPOSITORY_NOTES", "remote+local"),
    ("HARNESS_MEMORY_GLOBAL_NOTES", "remote+local"),
    ("HARNESS_MEMORY_SCRIPTS", "remote+local"),
];

/// A server that lists `pull-json` but neither `push-remote-local` nor `scripts-pull-json`.
const PULLS_NOTES_ONLY: Vars = &[
    ("FAKE_MEMORY_FEATURES", "push-json,pull-json"),
    ("HARNESS_MEMORY_REPOSITORY_NOTES", "remote+local"),
    ("HARNESS_MEMORY_GLOBAL_NOTES", "remote+local"),
];
const NO_FEATURES: Vars = &[("FAKE_MEMORY_FEATURES", "none")];
const OLDER: Vars = &[("FAKE_MEMORY_PUSH", "usage")];

#[tokio::test]
async fn a_copy_to_remote_takes_the_scopes_kept_here_and_never_names_a_scope() {
    let setup = Setup::installed(CURRENT, KEPT_HERE);
    let cache = NotesCopyCache::new();
    let to_remote = target(NOTES, TO_REMOTE);

    let review = try_preview_target(&setup, &cache, to_remote, ON)
        .await
        .expect("reviewed");
    assert_eq!((review.total, review.counts), (7, REVIEWED));

    let picked = LocalNoteIds::Ids(vec![1, 4]);
    let copied = try_apply_target(
        &setup,
        &cache,
        to_remote,
        ON,
        &CopyPicks::picking(&review.preview_id, &picked),
    )
    .await
    .expect("copied");
    assert_eq!(copied.deleted, 2);

    let notes = copies(&setup);
    assert_eq!(
        notes[0],
        ["push", "--remote-local", "--dry-run", "--json", "--overwrite", "none"]
    );
    assert_eq!(
        notes[1],
        [
            "push",
            "--remote-local",
            "--json",
            "--selection",
            "-",
            "--delete-copied"
        ]
    );
    assert_eq!(
        stdin_of(&setup, 1).as_deref(),
        Some(selection(&[NOTE_ENTRIES[0], NOTE_ENTRIES[3]], &[4]).as_str()),
        "the picked rows with their title and update time, the picked conflict to overwrite"
    );

    let scripts = target(SCRIPTS, TO_REMOTE);
    let review = try_preview_target(&setup, &cache, scripts, ON).await.expect("reviewed");
    let all = LocalNoteIds::Word(NoteIdsWord::All);
    try_apply_target(
        &setup,
        &cache,
        scripts,
        ON,
        &CopyPicks::picking(&review.preview_id, &all),
    )
    .await
    .expect("copied");

    let all_runs = copies(&setup);
    assert_eq!(
        all_runs[2],
        [
            "scripts",
            "push",
            "--remote-local",
            "--dry-run",
            "--json",
            "--overwrite",
            "none"
        ]
    );
    assert_eq!(
        all_runs[3],
        [
            "scripts",
            "push",
            "--remote-local",
            "--json",
            "--selection",
            "-",
            "--delete-copied"
        ]
    );
    assert_eq!(
        stdin_of(&setup, 3).as_deref(),
        Some(
            selection(
                &[
                    SCRIPT_ENTRIES[0],
                    SCRIPT_ENTRIES[2],
                    SCRIPT_ENTRIES[3],
                    SCRIPT_ENTRIES[5],
                    SCRIPT_ENTRIES[6]
                ],
                &[13, 14]
            )
            .as_str()
        ),
        "all is the review's pickable rows, not the identical or team-newer ones"
    );
    assert!(all_runs.iter().flatten().all(|arg| arg != "--scope"), "{all_runs:?}");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_copy_to_remote_takes_nothing_from_a_scope_routed_to_the_remote() {
    let setup = Setup::installed(CURRENT, &[]);
    let review = try_preview_target(&setup, &NotesCopyCache::new(), target(NOTES, TO_REMOTE), ON)
        .await
        .expect("reviewed");

    assert_eq!((review.total, review.rows.len()), (0, 0));
    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_stranded_copy_stays_plain_whatever_the_switches_say() {
    let setup = Setup::installed(CURRENT, KEPT_HERE);
    let cache = NotesCopyCache::new();

    for switches in [OFF, ON] {
        let review = try_preview_target(&setup, &cache, NOTES.into(), switches)
            .await
            .expect("reviewed");
        assert_eq!(review.total, 0, "the server skips what it keeps here");
    }

    assert_eq!(
        setup.runs().len(),
        2,
        "the server's features were not even asked: {:?}",
        copies(&setup)
    );
    assert!(
        copies(&setup)
            .iter()
            .all(|args| args == &["push", "--dry-run", "--json", "--overwrite", "none"])
    );
    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_direction_that_is_switched_off_is_refused_before_the_server_runs() {
    let setup = Setup::installed(CURRENT, KEPT_HERE);

    let wrong_switch = |mode| match mode {
        TO_REMOTE => Switches {
            to_remote: false,
            to_local: true,
        },
        _ => Switches {
            to_remote: true,
            to_local: false,
        },
    };

    for kind in [NOTES, SCRIPTS] {
        for mode in [TO_REMOTE, TO_LOCAL] {
            for switches in [OFF, wrong_switch(mode)] {
                let refused = try_preview_target(&setup, &NotesCopyCache::new(), target(kind, mode), switches).await;

                assert!(
                    matches!(refused, Err(BusinessError::Forbidden(_))),
                    "{kind:?} {mode:?}: {refused:?}"
                );
            }
        }
    }

    assert!(setup.requested().is_empty(), "nothing ran: {:?}", setup.requested());
    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_copy_is_refused_when_its_switch_is_turned_off_after_the_review_and_the_review_stays() {
    let setup = Setup::installed(CURRENT, KEPT_HERE);
    let cache = NotesCopyCache::new();

    for mode in [TO_REMOTE, TO_LOCAL] {
        let copy = target(NOTES, mode);
        let review = try_preview_target(&setup, &cache, copy, ON).await.expect("reviewed");
        let runs = copies(&setup).len();

        let all = LocalNoteIds::Word(NoteIdsWord::All);
        let picks = CopyPicks::picking(&review.preview_id, &all);

        let refused = try_apply_target(&setup, &cache, copy, OFF, &picks).await;
        assert!(
            matches!(refused, Err(BusinessError::Forbidden(_))),
            "{mode:?}: {refused:?}"
        );
        assert_eq!(copies(&setup).len(), runs, "{mode:?}: no copy ran");

        try_apply_target(&setup, &cache, copy, ON, &picks)
            .await
            .unwrap_or_else(|error| panic!("{mode:?}: the review was kept: {error:?}"));
    }

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_server_without_the_feature_of_a_direction_is_unsupported_and_runs_no_copy() {
    let notes_only = Setup::installed(CURRENT, PULLS_NOTES_ONLY);
    let cache = NotesCopyCache::new();

    for (kind, mode) in [(NOTES, TO_REMOTE), (SCRIPTS, TO_REMOTE), (SCRIPTS, TO_LOCAL)] {
        let refused = try_preview_target(&notes_only, &cache, target(kind, mode), ON).await;
        assert!(
            matches!(refused, Err(BusinessError::Unsupported(_))),
            "{kind:?} {mode:?}: {refused:?}"
        );
    }

    assert!(copies(&notes_only).is_empty(), "{:?}", copies(&notes_only));

    try_preview_target(&notes_only, &cache, target(NOTES, TO_LOCAL), ON)
        .await
        .expect("pull-json is listed");
    assert_eq!(copies(&notes_only), [["pull", "--dry-run", "--json"]]);
    notes_only.scripts.shutdown().await;

    let older = Setup::installed(CURRENT, NO_FEATURES);
    let refused = try_preview_target(&older, &cache, target(NOTES, TO_LOCAL), ON).await;
    assert!(matches!(refused, Err(BusinessError::Unsupported(_))), "{refused:?}");
    older.scripts.shutdown().await;

    let missing = Setup::without_server(CURRENT, &[]);
    let refused = try_preview_target(&missing, &cache, target(NOTES, TO_LOCAL), ON).await;
    assert!(matches!(refused, Err(BusinessError::NotFound(_))), "{refused:?}");
    assert!(missing.requested().is_empty());
}

#[tokio::test]
async fn a_server_that_cannot_run_the_command_is_unsupported() {
    let older = Setup::installed(CURRENT, OLDER);

    for mode in [TO_REMOTE, TO_LOCAL] {
        let refused = try_preview_target(&older, &NotesCopyCache::new(), target(NOTES, mode), ON).await;

        assert!(
            matches!(refused, Err(BusinessError::Unsupported(_))),
            "{mode:?}: {refused:?}"
        );
    }

    older.scripts.shutdown().await;
}

#[tokio::test]
async fn a_review_is_never_used_for_another_kind_or_direction() {
    let setup = Setup::installed(CURRENT, KEPT_HERE);
    let cache = NotesCopyCache::new();

    let stranded = try_preview_target(&setup, &cache, NOTES.into(), ON)
        .await
        .expect("reviewed");
    let to_remote = try_preview_target(&setup, &cache, target(NOTES, TO_REMOTE), ON)
        .await
        .expect("reviewed");
    let to_local = try_preview_target(&setup, &cache, target(NOTES, TO_LOCAL), ON)
        .await
        .expect("reviewed");
    let runs = copies(&setup).len();

    for (copy, id) in [
        (target(NOTES, TO_REMOTE), &stranded.preview_id),
        (target(NOTES, TO_LOCAL), &to_remote.preview_id),
        (target(NOTES, NotesCopyMode::Stranded), &to_local.preview_id),
        (target(SCRIPTS, TO_REMOTE), &to_remote.preview_id),
    ] {
        let pick = LocalNoteIds::Word(NoteIdsWord::All);
        let picks = CopyPicks::picking(id, &pick);

        assert!(
            is_invalid(&cache.rows(copy, id, NotesCopyFilter::All, None, None)),
            "{copy:?}"
        );
        assert!(
            is_invalid(&try_apply_target(&setup, &cache, copy, ON, &picks).await),
            "{copy:?}"
        );
    }

    assert_eq!(copies(&setup).len(), runs, "no copy ran for a review of another target");
    assert_eq!(
        cache
            .rows(
                target(NOTES, TO_LOCAL),
                &to_local.preview_id,
                NotesCopyFilter::All,
                None,
                None
            )
            .expect("its own")
            .total,
        6
    );
    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_server_without_push_selection_cannot_copy_to_the_remote() {
    let setup = Setup::installed(CURRENT, WITHOUT_SELECTION);
    let cache = NotesCopyCache::new();

    for kind in [NOTES, SCRIPTS] {
        let refused = try_preview_target(&setup, &cache, target(kind, TO_REMOTE), ON).await;

        assert!(
            matches!(refused, Err(BusinessError::Unsupported(_))),
            "{kind:?}: {refused:?}"
        );
    }

    assert!(copies(&setup).is_empty(), "no copy ran: {:?}", copies(&setup));

    try_preview_target(&setup, &cache, target(NOTES, TO_LOCAL), ON)
        .await
        .expect("a copy to this computer needs no selection");
    setup.scripts.shutdown().await;
}
