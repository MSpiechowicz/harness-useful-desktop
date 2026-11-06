//! A change to the repositories turned on: only the one added is resolved, the ones listed stay as
//! they are, a removal stops saving for the project it names, and an entry with a link in it names
//! nothing, as on the server (see `saving_root_links.rs` for one a `git pull` brought).

use pretty_assertions::assert_eq;

use super::saving_root_cases::git_ok as git;
use super::*;

pub(super) fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(super) fn add_repo(data: &Path, kind: MemoryGuardKind, repo: &Path) -> MemoryGuards {
    let add = MemoryGuardsPatch {
        add_repo: Some(text(repo)),
        ..patch(kind)
    };

    update_global(data, &add).unwrap()
}

pub(super) fn remove_repo(data: &Path, kind: MemoryGuardKind, repo: &Path) -> MemoryGuards {
    let remove = MemoryGuardsPatch {
        remove_repo: Some(text(repo)),
        ..patch(kind)
    };

    update_global(data, &remove).unwrap()
}

#[test]
fn a_listed_folder_stays_as_it_is_when_a_folder_above_it_becomes_a_repository() {
    let (dir, data, _repo) = setup();
    let parent = dunce::canonicalize(dir.path()).unwrap().join("parent");
    let listed = parent.join("L");
    std::fs::create_dir_all(&listed).unwrap();

    let added = add_repo(&data, MemoryGuardKind::Notes, &listed);
    assert_eq!(added.notes.on_repos, [text(&listed)]);

    // `git init` in the parent: resolving the plain folder `L` again would now give the parent.
    std::fs::create_dir(parent.join(".git")).unwrap();

    for patch in [
        MemoryGuardsPatch {
            paused: Some(true),
            ..patch(MemoryGuardKind::Notes)
        },
        MemoryGuardsPatch {
            every_project: Some(true),
            ..patch(MemoryGuardKind::Scripts)
        },
    ] {
        assert_eq!(
            update_global(&data, &patch).unwrap().notes.on_repos,
            [text(&listed)],
            "{patch:?}"
        );
    }

    let other = dunce::canonicalize(dir.path()).unwrap().join("other");
    std::fs::create_dir_all(other.join(".git")).unwrap();
    let mut expected = vec![text(&listed), text(&other)];
    expected.sort();
    assert_eq!(add_repo(&data, MemoryGuardKind::Notes, &other).notes.on_repos, expected);
}

#[test]
fn a_removal_of_a_folder_that_became_a_file_removes_the_entry_written_as_given() {
    let (dir, data, _repo) = setup();
    let gone = dunce::canonicalize(dir.path()).unwrap().join("was-a-folder");

    add_repo(&data, MemoryGuardKind::Notes, &gone);
    assert_eq!(read_global(&data).unwrap().notes.on_repos, [text(&gone)]);

    std::fs::write(&gone, "now a file").unwrap();

    assert!(
        remove_repo(&data, MemoryGuardKind::Notes, &gone)
            .notes
            .on_repos
            .is_empty()
    );
}

/// A checkout with its git folder kept elsewhere and a worktree of it, in `base`, as canonical
/// `(checkout, git folder, worktree)`.
fn separate_git_folder(base: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let checkout = base.join("checkout");
    let git_dir = base.join("checkout.git");
    let worktree = base.join("worktree");

    git(
        base,
        &[
            "init",
            "-q",
            "-b",
            "main",
            "--separate-git-dir",
            git_dir.to_str().unwrap(),
            checkout.to_str().unwrap(),
        ],
    );
    git(&checkout, &["commit", "-q", "--allow-empty", "-m", "first"]);
    git(
        &checkout,
        &["worktree", "add", "-q", "-b", "side", worktree.to_str().unwrap()],
    );

    (
        dunce::canonicalize(checkout).unwrap(),
        dunce::canonicalize(git_dir).unwrap(),
        dunce::canonicalize(worktree).unwrap(),
    )
}

#[test]
fn a_removal_through_a_worktree_removes_every_entry_that_turns_its_repository_on() {
    let (dir, data, _repo) = setup();
    let (checkout, git_dir, worktree) = separate_git_folder(&dunce::canonicalize(dir.path()).unwrap());

    // Both the git folder and the checkout that keeps it there are listed.
    add_repo(&data, MemoryGuardKind::Notes, &git_dir);
    add_repo(&data, MemoryGuardKind::Notes, &checkout);
    add_repo(&data, MemoryGuardKind::Scripts, &checkout);
    assert_eq!(read_global(&data).unwrap().notes.on_repos.len(), 2);

    let removed = remove_repo(&data, MemoryGuardKind::Notes, &worktree);

    assert!(removed.notes.on_repos.is_empty(), "{:?}", removed.notes.on_repos);
    assert_eq!(
        removed.scripts.on_repos,
        [text(&checkout)],
        "the other kind is not touched"
    );
}

#[test]
fn a_removal_of_a_checkout_with_a_separate_git_folder_removes_the_git_folder_entry_too() {
    let (dir, data, _repo) = setup();
    let (checkout, git_dir, worktree) = separate_git_folder(&dunce::canonicalize(dir.path()).unwrap());

    // Both the checkout and the git folder it keeps elsewhere are listed for the notes.
    add_repo(&data, MemoryGuardKind::Notes, &checkout);
    add_repo(&data, MemoryGuardKind::Notes, &git_dir);
    add_repo(&data, MemoryGuardKind::Scripts, &git_dir);
    assert!(repo_is_on(&worktree, &read_global(&data).unwrap().notes.on_repos));

    // Turning off the checkout leaves no entry that keeps its worktree on.
    let removed = remove_repo(&data, MemoryGuardKind::Notes, &checkout);

    assert!(removed.notes.on_repos.is_empty(), "{:?}", removed.notes.on_repos);
    assert!(!repo_is_on(&worktree, &removed.notes.on_repos));
    assert_eq!(
        removed.scripts.on_repos,
        [text(&git_dir)],
        "the other kind is not touched"
    );
}

#[test]
fn turning_off_from_a_worktree_removes_the_checkout_that_keeps_the_git_folder_it_sends() {
    let (dir, data, _repo) = setup();
    let (checkout, git_dir, worktree) = separate_git_folder(&dunce::canonicalize(dir.path()).unwrap());

    // Adding from the worktree stores the git folder, adding from the checkout stores the checkout.
    let from_worktree = add_repo(&data, MemoryGuardKind::Notes, &worktree);
    assert_eq!(from_worktree.notes.on_repos, [text(&git_dir)]);
    add_repo(&data, MemoryGuardKind::Notes, &checkout);
    add_repo(&data, MemoryGuardKind::Scripts, &checkout);
    assert!(repo_is_on(&worktree, &read_global(&data).unwrap().notes.on_repos));

    // The worktree's row sends the entry it turns on: the git folder.
    let removed = remove_repo(&data, MemoryGuardKind::Notes, &git_dir);

    assert!(removed.notes.on_repos.is_empty(), "{:?}", removed.notes.on_repos);
    assert!(!repo_is_on(&worktree, &removed.notes.on_repos));
    assert_eq!(
        removed.scripts.on_repos,
        [text(&checkout)],
        "the other kind is not touched"
    );
}

#[cfg(unix)]
#[test]
fn a_git_folder_reached_through_a_link_is_listed_canonical_and_an_entry_that_is_a_link_names_nothing() {
    let (dir, data, repo) = setup();
    let base = dunce::canonicalize(dir.path()).unwrap();
    let git_dir = base.join("bare.git");
    std::fs::create_dir_all(git_dir.join("objects")).unwrap();
    std::fs::create_dir_all(git_dir.join("refs")).unwrap();
    std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();

    let link = base.join("bare-link.git");
    std::os::unix::fs::symlink(&git_dir, &link).unwrap();
    assert_eq!(
        add_repo(&data, MemoryGuardKind::Notes, &link).notes.on_repos,
        [text(&git_dir)]
    );

    // An entry that is a link names nothing, as on the server: it turns on neither its target nor
    // anything else.
    let entry = base.join("entry-link");
    std::os::unix::fs::symlink(&repo, &entry).unwrap();
    assert!(!repo_is_on(&repo, &[text(&entry)]));
    assert!(!repo_is_on(&entry, &[text(&entry)]));
    assert!(!repo_is_on(&base.join("elsewhere"), &[text(&entry)]));
    assert!(repo_is_on(&repo, &[text(&repo)]));
}
