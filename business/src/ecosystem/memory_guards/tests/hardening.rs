//! The opt-in marker, the lock around `global.json`, the size cap, a version 1 file that cannot be
//! moved yet, and the listing of a checkout with a separate git folder.

use std::fs;

use pretty_assertions::assert_eq;

use super::*;
use crate::ecosystem::memory_guards::lock::FileLock;
use crate::env::Env;

pub(super) const V1: &str =
    r#"{"version":1,"notes":{"paused":true,"offRepos":["/work/a"]},"scripts":{"paused":false,"offRepos":[]}}"#;

fn marker(data: &Path) -> PathBuf {
    data.join(GUARDS_DIR).join(OPT_IN_MARKER)
}

fn add(kind: MemoryGuardKind, repo: &str) -> MemoryGuardsPatch {
    MemoryGuardsPatch {
        kind,
        paused: None,
        every_project: None,
        add_repo: Some(repo.to_owned()),
        remove_repo: None,
    }
}

#[test]
fn the_marker_is_written_first_whatever_the_state_of_the_file() {
    let broken = [
        None,
        Some(V1.to_owned()),
        Some(r#"{"version":2,"notes":{"paused":false,"everyProject":true,"onRepos":[],"repoKeys":{}},"scripts":{"paused":false,"everyProject":false,"onRepos":[],"repoKeys":{}}}"#.to_owned()),
        Some("not json".to_owned()),
    ];

    for file in broken {
        let (_dir, data, _repo) = setup();
        fs::create_dir_all(data.join(GUARDS_DIR)).unwrap();

        if let Some(file) = &file {
            fs::write(global_path(&data), file).unwrap();
        }

        ensure_global(&data).unwrap();

        assert_eq!(
            fs::read_to_string(marker(&data)).unwrap(),
            r#"{"version":1}"#,
            "{file:?}"
        );

        #[cfg(unix)]
        assert_eq!(mode(&marker(&data)), 0o600);

        let again = fs::read(marker(&data)).unwrap();
        ensure_global(&data).unwrap();
        assert_eq!(fs::read(marker(&data)).unwrap(), again, "a second run keeps it");
    }
}

#[test]
fn a_marker_that_cannot_be_written_fails_even_when_the_file_is_current() {
    let (dir, data, _repo) = setup();
    ensure_global(&data).unwrap();
    let current = fs::read(global_path(&data)).unwrap();

    // A folder where the marker belongs: nothing can be renamed over it.
    fs::remove_file(marker(&data)).unwrap();
    fs::create_dir(marker(&data)).unwrap();
    assert!(ensure_global(&data).is_err());
    assert_eq!(fs::read(global_path(&data)).unwrap(), current);

    #[cfg(unix)]
    {
        fs::remove_dir(marker(&data)).unwrap();
        let elsewhere = dir.path().join("elsewhere");
        fs::write(&elsewhere, "keep").unwrap();
        std::os::unix::fs::symlink(&elsewhere, marker(&data)).unwrap();

        assert!(matches!(ensure_global(&data), Err(BusinessError::InvalidInput(_))));
        assert_eq!(fs::read_to_string(&elsewhere).unwrap(), "keep", "not written through");
    }

    let _ = dir;
}

#[test]
fn a_version_one_file_that_cannot_be_rewritten_shows_as_pending_and_heals_when_it_can() {
    let (_dir, data, _repo) = setup();
    fs::create_dir_all(data.join(GUARDS_DIR)).unwrap();
    fs::write(global_path(&data), V1).unwrap();

    // The marker cannot be written, so the move fails.
    fs::create_dir(marker(&data)).unwrap();
    assert!(ensure_global(&data).is_err());

    let shown = read_global_healed(&Env::for_tests(data.parent().unwrap()), &data).unwrap();
    assert!(shown.migration_pending);
    assert!(shown.notes.paused);
    assert_eq!(fs::read_to_string(global_path(&data)).unwrap(), V1, "still version 1");

    fs::remove_dir(marker(&data)).unwrap();

    let healed = read_global_healed(&Env::for_tests(data.parent().unwrap()), &data).unwrap();
    assert!(!healed.migration_pending);
    assert!(healed.notes.paused, "the pause stayed");

    let file: serde_json::Value = serde_json::from_str(&fs::read_to_string(global_path(&data)).unwrap()).unwrap();
    assert_eq!(file["version"], 2);

    // Never written to disk: the file is exactly version, notes and scripts.
    let mut keys: Vec<&String> = file.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["notes", "scripts", "version"]);
    assert!(!read_global(&data).unwrap().migration_pending);
}

#[test]
fn changes_made_through_separate_lock_handles_all_apply() {
    let (_dir, data, _repo) = setup();
    ensure_global(&data).unwrap();
    let guards = data.join(GUARDS_DIR);

    // Each thread takes the file lock on its own handle and skips the mutex of this process, as
    // two processes do.
    let workers: Vec<_> = (0..4)
        .map(|worker| {
            let (data, guards) = (data.clone(), guards.clone());

            std::thread::spawn(move || {
                for index in 0..10 {
                    let _lock = FileLock::take(&guards, std::time::Duration::from_secs(30)).unwrap();
                    let patch = add(MemoryGuardKind::Notes, &format!("/gone/{worker}/{index}"));
                    change::apply(&data, &patch, Some(Pin(None))).unwrap();
                }
            })
        })
        .collect();

    for worker in workers {
        worker.join().unwrap();
    }

    assert_eq!(read_global(&data).unwrap().notes.on_repos.len(), 40);
}

#[test]
fn a_lock_held_elsewhere_is_busy_after_the_wait_and_the_file_is_untouched() {
    let (_dir, data, _repo) = setup();
    ensure_global(&data).unwrap();
    let before = fs::read(global_path(&data)).unwrap();
    let guards = data.join(GUARDS_DIR);

    let held = FileLock::take(&guards, std::time::Duration::from_secs(1)).unwrap();
    let started = std::time::Instant::now();
    let busy = FileLock::take(&guards, std::time::Duration::from_millis(150));

    assert!(matches!(busy, Err(BusinessError::Protocol(_))));
    assert!(started.elapsed() < std::time::Duration::from_secs(4));
    assert_eq!(fs::read(global_path(&data)).unwrap(), before);

    drop(held);
    update_global(&data, &add(MemoryGuardKind::Notes, "/gone/after")).unwrap();

    #[cfg(unix)]
    assert_eq!(mode(&guards.join("global.lock")), 0o600);
}

#[cfg(unix)]
#[test]
fn the_lock_file_is_never_opened_through_a_symbolic_link() {
    let (dir, data, _repo) = setup();
    ensure_global(&data).unwrap();
    let elsewhere = dir.path().join("elsewhere");
    fs::write(&elsewhere, "keep").unwrap();
    let lock = data.join(GUARDS_DIR).join("global.lock");
    fs::remove_file(&lock).ok();
    std::os::unix::fs::symlink(&elsewhere, &lock).unwrap();

    assert!(update_global(&data, &add(MemoryGuardKind::Notes, "/gone/x")).is_err());
    assert_eq!(fs::read_to_string(&elsewhere).unwrap(), "keep");
}

#[test]
fn settings_over_the_size_cap_are_refused_and_the_file_stays_byte_for_byte() {
    let (_dir, data, _repo) = setup();
    ensure_global(&data).unwrap();
    let mut refused = None;

    for index in 0..MAX_ON_REPOS {
        let before = fs::read(global_path(&data)).unwrap();
        let kind = if index % 2 == 0 {
            MemoryGuardKind::Notes
        } else {
            MemoryGuardKind::Scripts
        };
        // Nineteen folders of about 200 characters: long, and valid file names.
        let folders = format!("/{}", "a".repeat(200)).repeat(18);
        let repo = format!("/m{index:04}{folders}");
        assert!(repo.chars().count() <= MAX_REPO_CHARS);

        match update_global(&data, &add(kind, &repo)) {
            Ok(_) => assert!(fs::metadata(global_path(&data)).unwrap().len() <= MAX_FILE_BYTES),
            Err(error) => {
                assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
                assert_eq!(fs::read(global_path(&data)).unwrap(), before, "left as it was");
                refused = Some(index);
                break;
            }
        }
    }

    assert!(
        refused.is_some_and(|index| index > 10),
        "the cap is reached after many long paths"
    );
    assert!(read_global(&data).is_ok(), "what is on disk still reads");
}

/// `main` keeps its git folder in `gd.git`, with a worktree `wt` (the layout git writes), and
/// `forged` only names `gd.git`.
fn separate_layout(base: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let main = base.join("main");
    let git_dir = base.join("gd.git");
    let admin = git_dir.join("worktrees").join("wt");
    let linked = base.join("wt");
    let forged = base.join("forged");

    for folder in [
        &main,
        &admin,
        &linked,
        &forged,
        &git_dir.join("objects"),
        &git_dir.join("refs"),
    ] {
        fs::create_dir_all(folder).unwrap();
    }

    fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();

    fs::write(main.join(".git"), format!("gitdir: {}\n", git_dir.display())).unwrap();
    fs::write(admin.join("commondir"), "../..\n").unwrap();
    fs::write(admin.join("gitdir"), format!("{}\n", linked.join(".git").display())).unwrap();
    fs::write(linked.join(".git"), format!("gitdir: {}\n", admin.display())).unwrap();
    fs::write(forged.join(".git"), format!("gitdir: {}\n", git_dir.display())).unwrap();

    (main, git_dir, linked, forged)
}

#[test]
fn listing_a_checkout_with_a_separate_git_folder_turns_on_its_worktrees_only() {
    let (dir, data, _repo) = setup();
    let base = dunce::canonicalize(dir.path()).unwrap();
    let (main, git_dir, linked, forged) = separate_layout(&base);
    let text = |path: &Path| path.to_string_lossy().into_owned();

    assert_eq!(saving_root(&linked), git_dir, "the worktree maps to the git folder");
    assert_eq!(saving_root(&main), main);
    assert_eq!(
        saving_root(&forged),
        forged,
        "a folder that only names it is its own root"
    );

    let listed = update_global(&data, &add(MemoryGuardKind::Notes, &text(&main))).unwrap();
    assert_eq!(listed.notes.on_repos, [text(&main)]);

    assert!(repo_is_on(&main, &listed.notes.on_repos));
    assert!(repo_is_on(&linked, &listed.notes.on_repos), "its worktree is on");
    assert!(!repo_is_on(&forged, &listed.notes.on_repos), "a forged folder is not");
    assert!(!repo_is_on(&base.join("other"), &listed.notes.on_repos));
    assert!(!repo_is_on(&git_dir, &["relative".to_owned()]));

    // The UI compares this with `onRepos`, so the worktree answers with the entry that is listed.
    assert_eq!(saving_root_listed(Some(&data), &linked), Some(text(&main)));
    assert_eq!(saving_root_listed(Some(&data), &main), Some(text(&main)));
    assert_eq!(saving_root_listed(Some(&data), &forged), Some(text(&forged)));
    assert_eq!(
        saving_root_listed(None, &linked),
        Some(text(&git_dir)),
        "no data folder"
    );

    // Adding from the worktree stores the git folder, which turns on the worktrees but not `main`.
    let _ = update_global(
        &data,
        &MemoryGuardsPatch {
            remove_repo: Some(text(&main)),
            ..add(MemoryGuardKind::Notes, &text(&linked))
        },
    )
    .unwrap();

    let guards = read_global(&data).unwrap();
    assert_eq!(guards.notes.on_repos, [text(&git_dir)]);
    assert!(repo_is_on(&linked, &guards.notes.on_repos));
    assert!(!repo_is_on(&main, &guards.notes.on_repos));
    assert_eq!(saving_root_listed(Some(&data), &linked), Some(text(&git_dir)));
}

#[test]
fn an_unreadable_guards_file_leaves_the_plain_saving_root() {
    let (dir, data, _repo) = setup();
    let base = dunce::canonicalize(dir.path()).unwrap();
    let (_main, git_dir, linked, _forged) = separate_layout(&base);
    fs::create_dir_all(data.join(GUARDS_DIR)).unwrap();
    fs::write(global_path(&data), "not json").unwrap();

    assert_eq!(
        saving_root_listed(Some(&data), &linked),
        Some(git_dir.to_string_lossy().into_owned())
    );
}

#[test]
fn waiters_in_this_process_share_one_deadline_instead_of_queueing() {
    let (_dir, data, _repo) = setup();
    let _held = GlobalLock::take(&data).unwrap();
    let started = std::time::Instant::now();

    let waiters: Vec<_> = (0..3)
        .map(|_| {
            let data = data.clone();
            std::thread::spawn(move || GlobalLock::take(&data).is_err())
        })
        .collect();

    for waiter in waiters {
        assert!(waiter.join().unwrap(), "busy, not a lock");
    }

    // Three waiters one after another would take 15 seconds.
    assert!(
        started.elapsed() < std::time::Duration::from_secs(9),
        "{:?}",
        started.elapsed()
    );
}
