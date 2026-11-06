//! The saving root: a git worktree counts as its main repository, a clone or a forged `.git` file
//! does not. Worktrees are made by a real `git worktree add`.

use std::process::Command;

use pretty_assertions::assert_eq;

use super::*;

fn git(dir: &Path, home: &Path, args: &[&str]) {
    let identity = [
        ("GIT_AUTHOR_NAME", "Demo User"),
        ("GIT_AUTHOR_EMAIL", "demo@example.com"),
        ("GIT_COMMITTER_NAME", "Demo User"),
        ("GIT_COMMITTER_EMAIL", "demo@example.com"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("LC_ALL", "C"),
    ];

    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .envs(identity)
        .env("HOME", home)
        .env("GIT_CONFIG_GLOBAL", home.join(".gitconfig"))
        .output()
        .expect("git runs");

    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A repository `main` with one commit, a linked worktree `linked` of it and a clone `clone`, in a
/// temporary folder (canonical paths).
struct Repos {
    _dir: tempfile::TempDir,
    main: PathBuf,
    linked: PathBuf,
    clone: PathBuf,
}

fn repos() -> Repos {
    let dir = tempfile::tempdir().unwrap();
    let base = dunce::canonicalize(dir.path()).unwrap();
    let home = base.join("home");
    let main = base.join("main");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&main).unwrap();

    git(&main, &home, &["init", "-q", "-b", "main"]);
    std::fs::write(main.join("a.txt"), "a\n").unwrap();
    git(&main, &home, &["add", "a.txt"]);
    git(&main, &home, &["commit", "-q", "-m", "first"]);

    let linked = base.join("linked");
    git(
        &main,
        &home,
        &["worktree", "add", "-q", "-b", "side", linked.to_str().unwrap()],
    );

    let clone = base.join("clone");
    git(
        &base,
        &home,
        &["clone", "-q", main.to_str().unwrap(), clone.to_str().unwrap()],
    );

    Repos {
        _dir: dir,
        main,
        linked: dunce::canonicalize(linked).unwrap(),
        clone: dunce::canonicalize(clone).unwrap(),
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn a_linked_worktree_is_its_main_repository_and_a_clone_is_not() {
    let repos = repos();

    assert_eq!(saving_root(&repos.linked), repos.main);
    assert_eq!(saving_root(&repos.main), repos.main);
    assert_eq!(saving_root(&repos.clone), repos.clone);

    let lonely = repos.main.parent().unwrap().join("lonely");
    std::fs::create_dir(&lonely).unwrap();
    assert_eq!(saving_root(&lonely), lonely, "a folder outside a repository is itself");
}

#[test]
fn a_folder_inside_a_worktree_resolves_to_the_main_repository() {
    let repos = repos();
    std::fs::create_dir(repos.linked.join("src")).unwrap();

    assert_eq!(
        saving_root_of_folder(&repos.linked.join("src")),
        Some(text(&repos.main))
    );
    assert_eq!(saving_root_of_folder(&repos.main.join("missing")), None);
}

#[test]
fn adding_or_removing_a_worktree_acts_on_the_main_repository() {
    let repos = repos();
    let (_dir, data, _repo) = setup();

    let add = MemoryGuardsPatch {
        add_repo: Some(text(&repos.linked)),
        ..patch(MemoryGuardKind::Notes)
    };
    assert_eq!(update_global(&data, &add).unwrap().notes.on_repos, [text(&repos.main)]);

    let both = MemoryGuardsPatch {
        add_repo: Some(text(&repos.clone)),
        ..patch(MemoryGuardKind::Notes)
    };
    let mut expected = vec![text(&repos.main), text(&repos.clone)];
    expected.sort();
    assert_eq!(
        update_global(&data, &both).unwrap().notes.on_repos,
        expected,
        "a clone is a repository of its own"
    );

    let remove = MemoryGuardsPatch {
        remove_repo: Some(text(&repos.linked)),
        ..patch(MemoryGuardKind::Notes)
    };
    assert_eq!(
        update_global(&data, &remove).unwrap().notes.on_repos,
        [text(&repos.clone)],
        "the worktree turns its main repository off"
    );

    let remove_listed = MemoryGuardsPatch {
        remove_repo: Some(text(&repos.clone)),
        ..patch(MemoryGuardKind::Notes)
    };
    assert!(update_global(&data, &remove_listed).unwrap().notes.on_repos.is_empty());
}

#[test]
fn a_forged_git_file_does_not_claim_another_repository() {
    let repos = repos();
    let git_dirs = repos.main.join(".git").join("worktrees");
    let real_admin = std::fs::read_dir(&git_dirs).unwrap().next().unwrap().unwrap().path();

    // A folder that points at the real worktree's admin folder, which points back at `linked`.
    let forged = repos.main.parent().unwrap().join("forged");
    std::fs::create_dir(&forged).unwrap();
    std::fs::write(forged.join(".git"), format!("gitdir: {}\n", real_admin.display())).unwrap();
    assert_eq!(saving_root(&forged), forged, "no back-link to the forged folder");

    // Pointing at a repository's own `.git` folder: no `commondir`, like a submodule.
    let bare_claim = repos.main.parent().unwrap().join("claim");
    std::fs::create_dir(&bare_claim).unwrap();
    std::fs::write(
        bare_claim.join(".git"),
        format!("gitdir: {}\n", repos.main.join(".git").display()),
    )
    .unwrap();
    assert_eq!(saving_root(&bare_claim), bare_claim);

    assert_eq!(saving_root(&repos.linked), repos.main, "the real worktree is untouched");
}

#[test]
fn a_relative_gitdir_and_trailing_newlines_are_read() {
    let repos = repos();
    let admin = std::fs::read_dir(repos.main.join(".git").join("worktrees"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let name = admin.file_name().unwrap().to_string_lossy().into_owned();

    // The worktree's `.git` file with a path relative to the worktree, and CRLF endings.
    let relative = format!("../main/.git/worktrees/{name}");
    std::fs::write(repos.linked.join(".git"), format!("gitdir: {relative}\r\n\n")).unwrap();
    assert_eq!(saving_root(&repos.linked), repos.main);

    // `commondir` and `gitdir` of the admin folder, absolute and with endings of either kind.
    std::fs::write(
        admin.join("commondir"),
        format!("{}\r\n", repos.main.join(".git").display()),
    )
    .unwrap();
    std::fs::write(
        admin.join("gitdir"),
        format!("{}\n", repos.linked.join(".git").display()),
    )
    .unwrap();
    assert_eq!(saving_root(&repos.linked), repos.main);
}

#[test]
fn a_file_that_is_too_large_or_has_several_lines_leaves_the_root_as_it_is() {
    let repos = repos();
    let marker = repos.linked.join(".git");
    let first = std::fs::read_to_string(&marker).unwrap();

    std::fs::write(&marker, format!("{first}gitdir: elsewhere\n")).unwrap();
    assert_eq!(saving_root(&repos.linked), repos.linked, "two lines");

    std::fs::write(&marker, format!("gitdir: {}", "x".repeat(32 * 1024))).unwrap();
    assert_eq!(saving_root(&repos.linked), repos.linked, "too large");

    std::fs::write(&marker, "").unwrap();
    assert_eq!(saving_root(&repos.linked), repos.linked, "empty");

    std::fs::remove_file(&marker).unwrap();
    assert_eq!(saving_root(&repos.linked), repos.linked, "no .git at all");
}
