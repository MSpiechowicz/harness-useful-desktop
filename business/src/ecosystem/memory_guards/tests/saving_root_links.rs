//! A listed folder that a `git pull` replaced by a tracked link turns on nothing. The user turned
//! on a submodule (or a nested clone) `U/sub`; upstream then commits a link `sub -> ../O` at that
//! path, and a default `git pull` exits 0 and leaves `U/sub` a link. The entry would canonicalise
//! to `O`, a repository the user never turned on, so an entry names something only when it is its
//! own canonical form ([`names_itself`]). Harness Memory tests the same layouts through the MCP
//! tools (`tests/mcp_guard_links.rs` in its repository): a change to a layout or to its answer
//! updates the other repository too.

use std::os::unix::fs::symlink;

use pretty_assertions::assert_eq;

use super::changes::{add_repo, remove_repo, text};
use super::saving_root_cases::git_ok as git;
use super::*;

/// Where upstream's link leads.
#[derive(Clone, Copy)]
enum Link {
    /// `sub -> ../O`, a relative target.
    Relative,
    /// `sub -> <O>`, an absolute target.
    Absolute,
    /// The entry is `U/libs/sub` and upstream turns the middle folder `libs` into `libs -> ../Op`,
    /// which holds the repository `sub`.
    MiddleFolder,
}

/// The names of the layouts, in step with Harness Memory's `LAYOUTS`.
const LAYOUTS: &[(&str, Link)] = &[
    ("relative", Link::Relative),
    ("absolute", Link::Absolute),
    ("middle-folder", Link::MiddleFolder),
];

/// A clone `clone` of `up`, with the entry `entry` listed, and the repository `off` the link will
/// lead to, which is not listed.
struct Pulled {
    _dir: tempfile::TempDir,
    data: PathBuf,
    up: PathBuf,
    clone: PathBuf,
    entry: PathBuf,
    off: PathBuf,
    link: Link,
}

fn repository(path: &Path) -> PathBuf {
    std::fs::create_dir_all(path).unwrap();
    git(path, &["init", "-q", "-b", "main"]);
    git(path, &["commit", "-q", "--allow-empty", "-m", "first"]);

    dunce::canonicalize(path).unwrap()
}

/// Upstream `up` holds the submodule `sub` (`libs/sub` for the middle folder), cloned to `U`, and
/// the entry is listed for the notes.
fn listed(link: Link) -> Pulled {
    let dir = tempfile::tempdir().unwrap();
    let base = dunce::canonicalize(dir.path()).unwrap();
    let data = base.join("data");

    let source = repository(&base.join("source"));
    let up = repository(&base.join("up"));
    let (at, off) = match link {
        Link::MiddleFolder => ("libs/sub", base.join("Op")),
        _ => ("sub", base.join("O")),
    };

    git(&up, &["submodule", "add", "-q", source.to_str().unwrap(), at]);
    git(&up, &["commit", "-q", "-m", "add the submodule"]);
    git(
        &base,
        &["clone", "-q", "--recurse-submodules", up.to_str().unwrap(), "U"],
    );

    // The repository the link will lead to: `O`, or `Op/sub` below a plain folder.
    let off = match link {
        Link::MiddleFolder => repository(&off.join("sub")),
        _ => repository(&off),
    };

    let clone = dunce::canonicalize(base.join("U")).unwrap();
    let entry = clone.join(at);
    let pulled = Pulled {
        _dir: dir,
        data,
        up,
        clone,
        entry,
        off,
        link,
    };

    add_repo(&pulled.data, MemoryGuardKind::Notes, &pulled.entry);
    pulled
}

impl Pulled {
    fn entries(&self) -> Vec<String> {
        read_global(&self.data).unwrap().notes.on_repos
    }

    /// Upstream replaces the submodule (or the middle folder) by a tracked link, and `U` pulls it
    /// with a default `git pull`, which exits 0 and leaves the entry's path behind a link.
    fn pull_the_link(&self) {
        let target = match self.link {
            Link::Relative => PathBuf::from("../O"),
            Link::Absolute => self.off.clone(),
            Link::MiddleFolder => PathBuf::from("../Op"),
        };
        let (removed, replaced) = match self.link {
            Link::MiddleFolder => ("libs/sub", "libs"),
            _ => ("sub", "sub"),
        };

        git(&self.up, &["rm", "-qf", removed]);
        let _ = std::fs::remove_dir_all(self.up.join(replaced));
        symlink(&target, self.up.join(replaced)).unwrap();
        git(&self.up, &["add", replaced]);
        git(&self.up, &["commit", "-q", "-m", "a link where the submodule was"]);

        git(&self.clone, &["pull", "-q"]);

        let at = self.clone.join(replaced);
        assert!(
            std::fs::symlink_metadata(&at).unwrap().is_symlink(),
            "the pull left a link"
        );
    }

    fn is_on(&self, root: &Path) -> bool {
        repo_is_on(root, &self.entries())
    }
}

#[test]
fn a_listed_folder_that_a_pull_replaced_by_a_link_turns_on_nothing() {
    for (name, link) in LAYOUTS {
        let pulled = listed(*link);

        // Before the pull: the entry is on, the repository the link will lead to is not.
        assert!(pulled.is_on(&pulled.entry), "{name}: before");
        assert!(!pulled.is_on(&pulled.off), "{name}: before");

        pulled.pull_the_link();

        assert_eq!(
            pulled.entries(),
            [text(&pulled.entry)],
            "{name}: the entry is still listed"
        );
        assert!(
            !pulled.is_on(&pulled.off),
            "{name}: the repository the link leads to stays off"
        );
        assert!(!pulled.is_on(&pulled.entry), "{name}: the link itself names nothing");

        // What the UI shows for the repository the link leads to is its own root, not the entry.
        assert_eq!(
            saving_root_listed(Some(&pulled.data), &pulled.off),
            Some(text(&pulled.off)),
            "{name}"
        );
    }
}

#[test]
fn the_entry_a_pull_turned_into_a_link_is_removed_alone() {
    for (name, link) in LAYOUTS {
        let pulled = listed(*link);
        pulled.pull_the_link();

        // The repository the link leads to is turned on by the user later: cleaning up the stale
        // entry through Settings must not turn it off.
        add_repo(&pulled.data, MemoryGuardKind::Notes, &pulled.off);
        assert!(pulled.is_on(&pulled.off), "{name}: on by its own entry");

        let removed = remove_repo(&pulled.data, MemoryGuardKind::Notes, &pulled.entry);

        assert_eq!(removed.notes.on_repos, [text(&pulled.off)], "{name}");
        assert!(pulled.is_on(&pulled.off), "{name}");
    }
}

#[test]
fn an_entry_stored_while_its_folder_was_gone_names_nothing_until_the_folder_is_itself() {
    let (dir, data, _repo) = setup();
    let base = dunce::canonicalize(dir.path()).unwrap();
    let ghost = base.join("ghost");

    add_repo(&data, MemoryGuardKind::Notes, &ghost);
    assert_eq!(
        read_global(&data).unwrap().notes.on_repos,
        [text(&ghost)],
        "stored as given"
    );

    // The folder comes back as a link to a repository that is not listed: still nothing.
    let off = repository(&base.join("off"));
    symlink(&off, &ghost).unwrap();
    assert!(!repo_is_on(&off, &[text(&ghost)]));

    // The folder comes back as itself: the entry names it.
    std::fs::remove_file(&ghost).unwrap();
    let ghost = repository(&ghost);
    assert!(repo_is_on(&ghost, &[text(&ghost)]));
    assert!(!repo_is_on(&off, &[text(&ghost)]));
}
