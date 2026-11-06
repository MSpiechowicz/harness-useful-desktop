//! The worktree cases both repositories answer the same way. Harness Memory tests the same list
//! (`tests/mcp_guard_worktree_cases.rs` in its repository, through the MCP tools): a change to
//! [`CASES`], to a case's layout or to its answer updates the other repository too.
//!
//! The answer of a case ([`Maps`]) is the folder's saving root: the main repository, the git
//! folder of a repository made with a separate git dir, a bare repository's folder, or the folder
//! itself. Real worktrees, the separate and the bare repository and the submodule are made by git.

use std::process::Command;

use pretty_assertions::assert_eq;

use super::*;

/// Where a case's folder maps to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Maps {
    /// The main repository (`main`).
    Main,
    /// The git folder `separate.git` of `git init --separate-git-dir`.
    Separate,
    /// The folder `bare.git` of a bare repository.
    Bare,
    /// The folder itself: it is not believed to be a worktree.
    Itself,
}

/// A case name, and where the folder maps to.
///
/// Keep in step with `CASES` of Harness Memory's `tests/mcp_guard_worktree_cases.rs`.
const CASES: &[(&str, Maps)] = &[
    ("main", Maps::Main),
    ("worktree-absolute", Maps::Main),
    ("worktree-relative", Maps::Main),
    ("forged-admin-inside", Maps::Itself),
    ("forged-admin-elsewhere", Maps::Itself),
    ("two-lines", Maps::Itself),
    ("crlf", Maps::Main),
    ("oversize", Maps::Itself),
    ("at-the-cap", Maps::Main),
    ("missing-commondir", Maps::Itself),
    ("bad-back-link", Maps::Itself),
    ("submodule", Maps::Itself),
    ("separate-worktree", Maps::Separate),
    ("separate-main", Maps::Itself),
    ("forged-git-folder", Maps::Itself),
    ("bare-worktree", Maps::Bare),
    ("listed-folder-names-off-checkout", Maps::Itself),
    ("forged-admin-in-listed-work-tree", Maps::Itself),
    ("not-a-git-folder", Maps::Itself),
];

/// The largest `.git` file read, in bytes (the same bound on both sides).
const CAP: usize = 16 * 1024;

fn git(dir: &Path, args: &[&str]) -> bool {
    let output = Command::new("git")
        .args(["-c", "protocol.file.allow=always"])
        .args(args)
        .current_dir(dir)
        .envs([
            ("GIT_AUTHOR_NAME", "Demo User"),
            ("GIT_AUTHOR_EMAIL", "demo@example.com"),
            ("GIT_COMMITTER_NAME", "Demo User"),
            ("GIT_COMMITTER_EMAIL", "demo@example.com"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("LC_ALL", "C"),
        ])
        .env("GIT_CONFIG_GLOBAL", dir.join("no-such-gitconfig"))
        .output()
        .expect("git runs");

    output.status.success()
}

pub(super) fn git_ok(dir: &Path, args: &[&str]) {
    assert!(git(dir, args), "git {args:?}");
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
}

/// A repository `main` with one commit, in a canonical temporary folder.
fn main_repository() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let base = dunce::canonicalize(dir.path()).unwrap();
    let main = base.join("main");
    std::fs::create_dir(&main).unwrap();

    git_ok(&main, &["init", "-q", "-b", "main"]);
    write(&main.join("a.txt"), "a\n");
    git_ok(&main, &["add", "a.txt"]);
    git_ok(&main, &["commit", "-q", "-m", "first"]);

    (dir, base, main)
}

/// The linked worktree `name` of `main` next to it, with the admin folder of it.
fn linked(main: &Path, base: &Path, name: &str) -> (PathBuf, PathBuf) {
    git_ok(main, &["worktree", "add", "-q", "-b", name, &format!("../{name}")]);

    (
        dunce::canonicalize(base.join(name)).unwrap(),
        main.join(".git").join("worktrees").join(name),
    )
}

/// The folder of case `name`, made in `base` for the repository `main`.
fn folder(name: &str, base: &Path, main: &Path) -> PathBuf {
    let shown = main.join(".git");

    match name {
        "main" => main.to_path_buf(),

        "worktree-absolute" => linked(main, base, name).0,

        // `git worktree add --relative-paths` (git 2.48 and later) writes these paths. An older
        // git gets the same layout by hand.
        "worktree-relative" => {
            let target = format!("../{name}");

            if git(
                main,
                &["worktree", "add", "-q", "--relative-paths", "-b", name, &target],
            ) {
                return dunce::canonicalize(base.join(name)).unwrap();
            }

            let (folder, admin) = linked(main, base, name);
            write(
                &folder.join(".git"),
                &format!("gitdir: ../main/.git/worktrees/{name}\n"),
            );
            write(&admin.join("gitdir"), &format!("../../../../{name}/.git\n"));
            write(&admin.join("commondir"), "../..\n");
            folder
        }

        // The folder holds everything: its own admin folder that points at the main repository.
        "forged-admin-inside" => {
            let folder = base.join(name);
            std::fs::create_dir_all(folder.join("admin")).unwrap();
            write(&folder.join(".git"), "gitdir: admin\n");
            write(&folder.join("admin").join("gitdir"), "../.git\n");
            write(
                &folder.join("admin").join("commondir"),
                &format!("{}\n", shown.display()),
            );
            folder
        }

        // An admin folder outside the main repository's `worktrees` folder, with a good back-link.
        "forged-admin-elsewhere" => {
            let folder = base.join(name);
            let admin = base.join("elsewhere-admin");
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::create_dir_all(&admin).unwrap();
            write(&folder.join(".git"), &format!("gitdir: {}\n", admin.display()));
            write(&admin.join("gitdir"), &format!("{}\n", folder.join(".git").display()));
            write(&admin.join("commondir"), &format!("{}\n", shown.display()));
            folder
        }

        "two-lines" => {
            let (folder, _) = linked(main, base, name);
            let first = std::fs::read_to_string(folder.join(".git")).unwrap();
            write(&folder.join(".git"), &format!("{first}gitdir: elsewhere\n"));
            folder
        }

        // Windows line ends in the `.git` file and in both files of the admin folder.
        "crlf" => {
            let (folder, admin) = linked(main, base, name);
            write(&folder.join(".git"), &format!("gitdir: {}\r\n", admin.display()));
            write(&admin.join("gitdir"), &format!("{}\r\n", folder.join(".git").display()));
            write(&admin.join("commondir"), &format!("{}\r\n", shown.display()));
            folder
        }

        // One byte over the cap, and exactly at it, padded with line ends that are trimmed.
        "oversize" | "at-the-cap" => {
            let (folder, admin) = linked(main, base, name);
            let line = format!("gitdir: {}", admin.display());
            let size = if name == "oversize" { CAP + 1 } else { CAP };
            write(
                &folder.join(".git"),
                &format!("{line}{}", "\n".repeat(size - line.len())),
            );
            folder
        }

        "missing-commondir" => {
            let (folder, admin) = linked(main, base, name);
            std::fs::remove_file(admin.join("commondir")).unwrap();
            folder
        }

        "bad-back-link" => {
            let (folder, admin) = linked(main, base, name);
            write(&admin.join("gitdir"), "/nowhere/.git\n");
            folder
        }

        // A submodule: its `.git` file names `.git/modules/<name>`, which has no `commondir`.
        "submodule" => {
            let source = base.join("source");
            std::fs::create_dir(&source).unwrap();
            git_ok(&source, &["init", "-q", "-b", "main"]);
            git_ok(&source, &["commit", "-q", "--allow-empty", "-m", "first"]);

            git_ok(main, &["submodule", "add", "-q", source.to_str().unwrap(), name]);
            dunce::canonicalize(main.join(name)).unwrap()
        }

        // A worktree of a repository whose git folder is kept elsewhere: `separate/.git` names
        // `separate.git`, which does not name `separate`. The main checkout is its own root.
        "separate-worktree" | "separate-main" => {
            let checkout = separate(base);

            if name == "separate-main" {
                return checkout;
            }

            git_ok(&checkout, &["worktree", "add", "-q", "-b", name, &format!("../{name}")]);
            dunce::canonicalize(base.join(name)).unwrap()
        }

        // A folder whose `.git` file names the separate git folder, with no back-link.
        "forged-git-folder" => {
            separate(base);
            let folder = base.join(name);
            std::fs::create_dir_all(&folder).unwrap();
            write(
                &folder.join(".git"),
                &format!("gitdir: {}\n", base.join("separate.git").display()),
            );
            dunce::canonicalize(&folder).unwrap()
        }

        // A worktree of a bare repository: its common folder is the bare folder.
        "bare-worktree" => {
            let bare = base.join("bare.git");
            git_ok(
                base,
                &["clone", "-q", "--bare", main.to_str().unwrap(), bare.to_str().unwrap()],
            );
            git_ok(&bare, &["worktree", "add", "-q", "-b", name, &format!("../{name}")]);
            dunce::canonicalize(base.join(name)).unwrap()
        }

        // The checkout `off` is not listed. The listed folder `listed` has a `.git` file that
        // names it (and so does not make `off` a git folder).
        "listed-folder-names-off-checkout" => {
            let off = base.join("off");
            std::fs::create_dir_all(&off).unwrap();
            git_ok(&off, &["init", "-q", "-b", "main"]);
            let off = dunce::canonicalize(&off).unwrap();

            std::fs::create_dir_all(base.join("listed")).unwrap();
            write(
                &base.join("listed").join(".git"),
                &format!("gitdir: {}\n", off.display()),
            );
            off
        }

        // `main` is listed and holds a `worktrees/y` folder with both link files as ordinary
        // content. The folder's `.git` names it, so the common folder is the listed work tree.
        "forged-admin-in-listed-work-tree" => {
            let folder = base.join(name);
            std::fs::create_dir_all(&folder).unwrap();
            let folder = dunce::canonicalize(&folder).unwrap();
            let admin = main.join("worktrees").join("y");
            std::fs::create_dir_all(&admin).unwrap();
            write(&admin.join("commondir"), "../..\n");
            write(&admin.join("gitdir"), &format!("{}\n", folder.join(".git").display()));
            write(&folder.join(".git"), &format!("gitdir: {}\n", admin.display()));
            folder
        }

        // A `.git` file with a good back-link whose common folder is the plain folder `plain`,
        // which is listed: it is shaped like no git folder.
        "not-a-git-folder" => {
            let folder = base.join(name);
            let admin = base.join("plain").join("worktrees").join("z");
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::create_dir_all(&admin).unwrap();
            let folder = dunce::canonicalize(&folder).unwrap();
            write(&admin.join("commondir"), "../..\n");
            write(&admin.join("gitdir"), &format!("{}\n", folder.join(".git").display()));
            write(
                &folder.join(".git"),
                &format!("gitdir: {}\n", dunce::canonicalize(&admin).unwrap().display()),
            );
            folder
        }

        other => panic!("unknown case {other}"),
    }
}

/// The main checkout `separate` of a repository whose git folder is `separate.git`, made once.
fn separate(base: &Path) -> PathBuf {
    let checkout = base.join("separate");

    if !checkout.exists() {
        let git_dir = base.join("separate.git");
        let arguments = [
            "init",
            "-q",
            "-b",
            "main",
            "--separate-git-dir",
            git_dir.to_str().unwrap(),
        ];
        git_ok(base, &[&arguments[..], &[checkout.to_str().unwrap()]].concat());
        write(&checkout.join("a.txt"), "a\n");
        git_ok(&checkout, &["add", "a.txt"]);
        git_ok(&checkout, &["commit", "-q", "-m", "first"]);
    }

    dunce::canonicalize(&checkout).unwrap()
}

#[test]
fn every_case_maps_to_the_main_repository_or_to_itself() {
    let (_dir, base, main) = main_repository();

    for (name, maps) in CASES {
        let folder = folder(name, &base, &main);
        let expected = match maps {
            Maps::Main => main.clone(),
            Maps::Separate => base.join("separate.git"),
            Maps::Bare => base.join("bare.git"),
            Maps::Itself => folder.clone(),
        };

        assert_eq!(saving_root(&folder), dunce::canonicalize(expected).unwrap(), "{name}");
    }
}

#[test]
fn a_forged_folder_is_stored_as_itself_and_never_as_the_enabled_repository() {
    let (_dir, base, main) = main_repository();
    let data_dir = base.join("data");

    for name in ["forged-admin-inside", "forged-admin-elsewhere", "forged-git-folder"] {
        let forged = folder(name, &base, &main);
        let add = MemoryGuardsPatch {
            add_repo: Some(text(&forged)),
            ..patch(MemoryGuardKind::Notes)
        };

        assert_eq!(
            update_global(&data_dir, &add).unwrap().notes.on_repos,
            [text(&forged)],
            "{name}"
        );

        let remove = MemoryGuardsPatch {
            remove_repo: Some(text(&forged)),
            ..patch(MemoryGuardKind::Notes)
        };
        assert!(update_global(&data_dir, &remove).unwrap().notes.on_repos.is_empty());
    }
}

/// A listed folder cannot turn on a root it merely names: the cases that try it stay off, while
/// the real worktrees, a listed git folder and a listed checkout of one still turn theirs on.
#[test]
fn a_listed_folder_cannot_vouch_for_another_repository() {
    let (_dir, base, main) = main_repository();
    let on = |entries: &[&Path], root: &Path| {
        let entries: Vec<String> = entries.iter().map(|entry| text(entry)).collect();
        repo_is_on(root, &entries)
    };

    let off = folder("listed-folder-names-off-checkout", &base, &main);
    assert!(!on(&[&base.join("listed")], &off), "a .git file naming a checkout");

    let forged = folder("forged-admin-in-listed-work-tree", &base, &main);
    assert!(!on(&[&main], &forged), "a forged admin folder in a listed work tree");
    assert!(on(&[&main], &main));

    let plain = folder("not-a-git-folder", &base, &main);
    assert!(!on(&[&base.join("plain")], &plain), "a plain folder is no git folder");

    // The checkout of a separate git folder turns on its worktrees (a git folder) and itself,
    // and a git folder listed does the same for its worktrees but not for the checkout.
    let worktree = folder("separate-worktree", &base, &main);
    let checkout = folder("separate-main", &base, &main);
    let git_dir = base.join("separate.git");
    assert!(on(&[&checkout], &worktree) && on(&[&checkout], &checkout));
    assert!(on(&[&git_dir], &worktree) && !on(&[&git_dir], &checkout));

    // A git folder only ever matches a root that is a worktree of it, not one that is itself.
    let itself = folder("forged-git-folder", &base, &main);
    assert!(!on(&[&checkout], &itself) && !on(&[&git_dir], &itself));
}

/// A git folder inside another repository's work tree is stored as itself, not as the outer
/// repository (whose nearest `.git` is above it), which would turn that repository on.
#[test]
fn a_git_folder_inside_another_repository_is_stored_as_itself() {
    let (_dir, base, main) = main_repository();
    let data_dir = base.join("data");
    let inner = main.join("vendor").join("inner.git");

    for folder in ["objects", "refs"] {
        std::fs::create_dir_all(inner.join(folder)).unwrap();
    }
    write(&inner.join("HEAD"), "ref: refs/heads/main\n");

    let add = MemoryGuardsPatch {
        add_repo: Some(text(&inner)),
        ..patch(MemoryGuardKind::Notes)
    };
    let guards = update_global(&data_dir, &add).unwrap();

    assert_eq!(guards.notes.on_repos, [text(&inner)]);
    assert!(
        !repo_is_on(&main, &guards.notes.on_repos),
        "the outer repository stays off"
    );
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// A named pipe in place of a file the check reads returns at once and leaves the root as it is.
#[cfg(unix)]
#[test]
fn a_named_pipe_in_place_of_a_link_file_cannot_hang() {
    for file in ["gitdir", "commondir"] {
        let (_dir, base, main) = main_repository();
        let folder = base.join("pipe");
        let admin = folder.join("admin");
        std::fs::create_dir_all(&admin).unwrap();
        write(&folder.join(".git"), "gitdir: admin\n");
        write(&admin.join("gitdir"), "../.git\n");
        write(&admin.join("commondir"), &format!("{}\n", main.join(".git").display()));
        std::fs::remove_file(admin.join(file)).unwrap();
        assert!(Command::new("mkfifo").arg(admin.join(file)).status().unwrap().success());

        let (sender, receiver) = std::sync::mpsc::channel();
        let asked = folder.clone();

        // On a thread, so a hang fails the test instead of stopping it.
        std::thread::spawn(move || {
            let _ = sender.send(saving_root(&asked));
        });

        let root = receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap_or_else(|_| panic!("a named pipe as {file} hangs"));
        assert_eq!(root, folder, "{file}");
    }
}
