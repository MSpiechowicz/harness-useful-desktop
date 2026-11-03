//! The branch lookup a chat starts with: used once by the first message, waited for while it runs,
//! never used for another folder, and only started for a new chat whose project has the step.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime};

use harness_business::hooks::{HookCapabilities, HookSet, seed_defaults};
use tauri::async_runtime::block_on;

use super::super::members::Members;
use super::branch_prefetch::{BranchPrefetch, FRESH_FOR, wanted_at_start};
use super::lookups::BranchLookup;
use super::tests_lookups::scene;
use crate::test_support::{TempDir, home_state, wait_until};

/// A lookup that counts its runs and answers `note` after `delay`.
fn counted(
    runs: &Arc<AtomicUsize>,
    note: &'static str,
    delay: Duration,
) -> impl Future<Output = Option<String>> + use<> {
    let runs = runs.clone();

    async move {
        runs.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(delay).await;

        Some(note.to_owned())
    }
}

fn runs() -> Arc<AtomicUsize> {
    Arc::new(AtomicUsize::new(0))
}

#[test]
fn a_note_looked_up_at_the_start_is_used_without_looking_again() {
    let (started, at_message) = (runs(), runs());
    let prefetch = BranchPrefetch::default();
    let folder = Path::new("/work");

    block_on(async {
        prefetch.start(folder, counted(&started, "behind", Duration::ZERO));
        tokio::time::sleep(Duration::from_millis(50)).await;
    });

    let (note, prefetched) = block_on(prefetch.note(folder, || counted(&at_message, "again", Duration::ZERO)));

    assert_eq!((note.as_deref(), prefetched), (Some("behind"), true));
    assert_eq!(
        (started.load(Ordering::SeqCst), at_message.load(Ordering::SeqCst)),
        (1, 0)
    );
}

#[test]
fn a_lookup_that_still_runs_is_waited_for() {
    let (started, at_message) = (runs(), runs());
    let prefetch = BranchPrefetch::default();
    let folder = Path::new("/work");

    let (note, prefetched) = block_on(async {
        prefetch.start(folder, counted(&started, "slow", Duration::from_millis(150)));

        prefetch
            .note(folder, || counted(&at_message, "again", Duration::ZERO))
            .await
    });

    assert_eq!((note.as_deref(), prefetched), (Some("slow"), true));
    assert_eq!(
        (started.load(Ordering::SeqCst), at_message.load(Ordering::SeqCst)),
        (1, 0)
    );
}

#[test]
fn without_a_started_lookup_the_note_is_looked_up_at_the_message() {
    let at_message = runs();
    let prefetch = BranchPrefetch::default();
    let folder = Path::new("/work");

    let (note, prefetched) = block_on(prefetch.note(folder, || counted(&at_message, "now", Duration::ZERO)));

    assert_eq!((note.as_deref(), prefetched), (Some("now"), false));
    assert_eq!(at_message.load(Ordering::SeqCst), 1);
}

#[test]
fn a_started_lookup_is_used_once() {
    let (started, later) = (runs(), runs());
    let prefetch = BranchPrefetch::default();
    let folder = Path::new("/work");

    block_on(async {
        prefetch.start(folder, counted(&started, "first", Duration::ZERO));
        prefetch.note(folder, || counted(&later, "never", Duration::ZERO)).await;
    });

    let (note, prefetched) = block_on(prefetch.note(folder, || counted(&later, "second", Duration::ZERO)));

    assert_eq!((note.as_deref(), prefetched), (Some("second"), false));
    assert_eq!(later.load(Ordering::SeqCst), 1);
}

#[test]
fn a_chat_that_moved_to_another_folder_does_not_get_the_note_of_the_old_one() {
    let at_message = runs();
    let prefetch = BranchPrefetch::default();

    block_on(async {
        prefetch.start(Path::new("/old"), counted(&runs(), "old", Duration::ZERO));
    });

    let (note, prefetched) = block_on(prefetch.note(Path::new("/new"), || counted(&at_message, "new", Duration::ZERO)));

    assert_eq!((note.as_deref(), prefetched), (Some("new"), false));
}

#[test]
fn a_note_that_went_stale_is_not_used() {
    let prefetch = BranchPrefetch::default();
    let folder = PathBuf::from("/work");

    block_on(async {
        prefetch.start(&folder, counted(&runs(), "old", Duration::ZERO));
    });

    assert!(
        prefetch.take(&folder, Instant::now() + FRESH_FOR).is_none(),
        "older than it may be"
    );
}

/// Where the work tree's remote-tracking `main` stands.
fn remote_main(work: &Path) -> Vec<u8> {
    let output = std::process::Command::new("git")
        .args(["-C", &work.to_string_lossy(), "rev-parse", "origin/main"])
        .output()
        .unwrap();

    output.stdout
}

#[test]
fn the_started_lookup_fetches_once_and_the_first_message_gets_that_note() {
    let scene = scene("branch-prefetch-real");
    scene.git(&scene.work, &["switch", "-q", "-c", "feat"]);
    scene.push_to_main(1);

    let prefetch = BranchPrefetch::default();
    let before = remote_main(&scene.work);

    block_on(async {
        prefetch.start(&scene.work, BranchLookup::of(&scene.state).note(scene.work.clone()));
    });

    // Another commit lands after the lookup was started: a lookup at the message would see 2.
    wait_until("the started lookup fetched", || remote_main(&scene.work) != before);
    std::fs::write(scene.other.join("late.txt"), "x\n").unwrap();
    scene.git(&scene.other, &["add", "-A"]);
    scene.git(&scene.other, &["commit", "-q", "-m", "Late change"]);
    scene.git(&scene.other, &["push", "-q", "origin", "main"]);

    let (note, prefetched) = block_on(prefetch.note(&scene.work, || async { None }));
    let note = note.expect("a note");

    assert!(prefetched);
    assert!(
        note.starts_with("feat is 1 commit behind origin/main (fetched now):"),
        "{note}"
    );
}

#[test]
fn only_a_new_chat_whose_project_has_the_step_looks_up_the_branch_when_it_starts() {
    let dir = TempDir::new("branch-prefetch-wanted");
    let state = home_state(&dir);

    assert!(!wanted_at_start(true, &HookSet::empty()), "no hooks");

    let available = HookCapabilities {
        memory_available: true,
        dashboard_available: true,
        code_graph_available: true,
        knowledge_available: true,
        skills_available: true,
    };
    let store = state.hooks.store().expect("a hooks folder");
    seed_defaults(&store, available, SystemTime::now()).expect("the family hooks are seeded");
    state.hooks.lock_cache().members = Some((
        Instant::now(),
        Members {
            memory_installed: true,
            memory_enabled: true,
            ..Members::default()
        },
    ));
    let hooks = block_on(state.hooks.hook_set(None));

    assert!(wanted_at_start(true, &hooks), "a new chat");
    assert!(!wanted_at_start(false, &hooks), "a resumed chat");
}
