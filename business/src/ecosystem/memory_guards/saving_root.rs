//! The root a save guard is kept under ([`saving_root`]): a git worktree counts as the checkout it
//! belongs to, or as the git folder when that belongs to no checkout, a separate clone of it does
//! not. Harness Memory applies the same rule ([`map`]) to the `project_path` of a save, so a
//! repository the user turned saving on for stays on in each of its worktrees. Whether a root is
//! on ([`repo_is_on`]) and what the UI compares a folder's root with ([`saving_root_listed`])
//! follow the same rule.

use std::path::{Path, PathBuf};

use crate::hooks::project_identity;
use crate::security::SafeReader;

/// The largest `.git` file, `gitdir`, or `commondir` file read (each holds one path).
const MAX_LINK_BYTES: u64 = 16 * 1024;

/// The prefix of the line a worktree's `.git` file holds.
const GITDIR_PREFIX: &str = "gitdir:";

/// What a project root stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Mapping {
    /// The root itself: not a verified worktree, or its common folder cannot be believed.
    Itself(PathBuf),
    /// A verified worktree of the checkout at this folder (its common folder is named `.git`).
    Checkout(PathBuf),
    /// A verified worktree of this git folder, which is not named `.git` and is shaped like one.
    GitFolder(PathBuf),
}

impl Mapping {
    fn path(&self) -> &Path {
        match self {
            Self::Itself(path) | Self::Checkout(path) | Self::GitFolder(path) => path,
        }
    }
}

/// What the project root `root` (see [`project_identity`]) stands for.
///
/// An entry of the list names something only when it is its own canonical form
/// ([`names_itself`]), before any of the folders below is read.
///
/// A linked worktree's `<root>/.git` is a file `gitdir: <X>` (`X` absolute or relative to `root`)
/// and `<X>/commondir` names the common git folder (absolute or relative to `X`). The worktree
/// must also point back: `<X>/gitdir` has to name `<root>/.git`, and `<X>` has to be a child of the
/// common folder's `worktrees` folder, so the files of a folder alone (a forged admin folder inside
/// it, or elsewhere with a good back-link) cannot claim another repository. Anything else (a
/// submodule, which has no `commondir`, a file that is missing, too large or unreadable) leaves
/// `root` as it is ([`Mapping::Itself`]).
///
/// - A common folder named `.git` belongs to the checkout that holds it: [`Mapping::Checkout`].
/// - Any other common folder (a repository made with `git init --separate-git-dir`, or a bare one)
///   leads to no checkout: the folder has no back-link to a work tree. It counts only when it is
///   shaped like a git folder ([`is_git_folder`]: `HEAD` a regular file, `objects` and `refs`
///   folders, no `.git` inside), then the worktree is a [`Mapping::GitFolder`], and so are all the
///   other worktrees of that repository. A work tree that merely holds a `worktrees/<name>` folder
///   with the two files is no git folder, so a forged one in a listed folder leaves the root
///   as itself.
///
/// A [`Mapping::GitFolder`] is on only by an entry that is that git folder, or by the entry of the
/// main checkout that keeps its git folder there ([`separate_git_dir`]: a `.git` file `gitdir: X`,
/// `X` shaped like a git folder with no `commondir`); such an entry never turns on a root that
/// maps to itself or to a checkout, so a listed folder cannot vouch for another repository by
/// naming it. An entry that is itself a verified worktree turns on its [`Mapping::Checkout`] and
/// its own path ([`repo_is_on`]).
///
/// What this does not stop: someone who can already write inside a repository's own git folder
/// (its `worktrees` folder, or a `.git` file in a listed checkout that names the git folder of
/// another repository) can still make a root that repository's worktree, and a listed checkout
/// still turns on the worktrees of the git folder it names. A plain folder that was delivered
/// without git (an archive, say) and that the user turned on, shaped like a git folder, can vouch
/// for a folder that has a planted `.git` file: the shape of a folder cannot prove it is a git
/// folder. That takes a way to put files there, an archive or a shell. With a separate git
/// folder, listing the git folder for one kind and the checkout for another can show a worktree
/// as on in the UI where the server saves.
///
/// Harness Memory applies the same rule (`src/mcp/worktree.rs` there) and both repositories test
/// the same cases (`tests/saving_root_cases.rs` here, `tests/mcp_guard_worktree_cases.rs` there):
/// a change to this rule updates the other repository and its cases.
pub(super) fn map(root: &Path) -> Mapping {
    let itself = || Mapping::Itself(root.to_path_buf());

    let Some(common) = linked_common(root) else {
        return itself();
    };

    if common.file_name().is_some_and(|name| name == ".git") {
        return common
            .parent()
            .map_or_else(itself, |main| Mapping::Checkout(main.to_path_buf()));
    }

    if is_git_folder(&common) {
        return Mapping::GitFolder(common);
    }

    itself()
}

/// The root to compare with the repositories saving is on for, for the project root `root`: its
/// [`map`] path, so `root` itself unless it is a linked git worktree, then the main checkout's
/// working folder or the git folder it belongs to.
pub fn saving_root(root: &Path) -> PathBuf {
    map(root).path().to_path_buf()
}

/// True when saving is on for the project root `root` (see [`project_identity`]) by one of the
/// `on_repos` the user turned on ([`Allowed`]). Entries are written by this app, so unlike a call
/// root they are believed, as long as each [names itself](names_itself). Only the path is
/// compared: Harness Memory also requires the project key of the entry's pin (`repoKeys`) to be
/// the repository's key now, which this app cannot compute, so a caller passes the entries that
/// have a pin ([`GlobalKindGuard::pinned_repos`](crate::model::GlobalKindGuard::pinned_repos)) and
/// learns of a changed key from `changedRepos`.
pub fn repo_is_on(root: &Path, on_repos: &[String]) -> bool {
    Allowed::of(on_repos.iter().map(Path::new)).allows(&map(root))
}

/// What the entries of `onRepos` turn on.
#[derive(Default)]
struct Allowed {
    /// Roots that are themselves an entry, and the checkouts of entries that are worktrees.
    paths: Vec<PathBuf>,
    /// The git folders whose worktrees are on: an entry that is one, and the git folder a listed
    /// checkout keeps elsewhere.
    git_folders: Vec<PathBuf>,
}

impl Allowed {
    /// What `entries` turn on. An entry that does not [name itself](names_itself) names nothing,
    /// before any of its folders is read.
    fn of<'a>(entries: impl IntoIterator<Item = &'a Path>) -> Self {
        let mut allowed = Self::default();

        for entry in entries.into_iter().filter(|entry| names_itself(entry)) {
            if let Mapping::Checkout(main) = map(entry) {
                allowed.paths.push(main);
            }

            if is_git_folder(entry) {
                allowed.git_folders.push(entry.to_path_buf());
            }

            if let Some(git_dir) = separate_git_dir(entry) {
                allowed.git_folders.push(git_dir);
            }

            allowed.paths.push(entry.to_path_buf());
        }

        allowed
    }

    fn allows(&self, mapping: &Mapping) -> bool {
        match mapping {
            Mapping::Itself(path) | Mapping::Checkout(path) => self.paths.contains(path),
            Mapping::GitFolder(git_dir) => self.git_folders.contains(git_dir),
        }
    }
}

/// True when `entry` is an absolute path that is its own canonical form: no link in it, now. The
/// app writes entries canonical ([`saving_root`]), so this holds for each entry it wrote for a
/// folder that is still there. An entry with a link in it (a listed folder that a `git pull`
/// replaced by a tracked link, or a folder moved and replaced by a link) would lead to a
/// repository the user never turned on, so it names nothing, and so does one whose folder is gone
/// (it names something again once its folder is back as itself). Harness Memory reads entries the
/// same way. The canonical form is compared exactly, as text, since it is the form that was
/// stored (`dunce::canonicalize`): a spelling that differs only by case or by a Windows drive
/// letter is a different path and names nothing, so a mismatch only ever turns saving off.
pub(super) fn names_itself(entry: &Path) -> bool {
    entry.is_absolute() && dunce::canonicalize(entry).is_ok_and(|real| real.as_os_str() == entry.as_os_str())
}

/// The git folder of the main checkout at `entry` when it keeps it elsewhere: its `.git` is a file
/// of one line `gitdir: X`, and `X` is shaped like a git folder ([`is_git_folder`]) with no
/// `commondir`, so it is no worktree's admin folder (canonical).
pub(super) fn separate_git_dir(entry: &Path) -> Option<PathBuf> {
    let git_dir = git_file_dir(entry)?;

    if !is_git_folder(&git_dir) || std::fs::symlink_metadata(git_dir.join("commondir")).is_ok() {
        return None;
    }

    Some(git_dir)
}

/// True when `path` is shaped like a git folder: `HEAD` a regular file, `objects` and `refs`
/// folders, and no `.git` inside (so a work tree is none, whatever else it holds).
pub(super) fn is_git_folder(path: &Path) -> bool {
    let kind = |name: &str| std::fs::symlink_metadata(path.join(name)).map(|metadata| metadata.file_type());

    kind("HEAD").is_ok_and(|kind| kind.is_file())
        && kind("objects").is_ok_and(|kind| kind.is_dir())
        && kind("refs").is_ok_and(|kind| kind.is_dir())
        && kind(".git").is_err()
}

/// What the UI compares with `onRepos` for `folder` (the saving root of its project): the saving
/// root `r` ([`saving_root_of_folder`]), or, when the guards of `data_dir` list an entry that
/// turns `r` on without being it (a checkout with a separate git folder, for one of its
/// worktrees), that entry. `r` when the guards cannot be read or the data folder is unknown. The
/// answer changes with the guards.
pub fn saving_root_listed(data_dir: Option<&Path>, folder: &Path) -> Option<String> {
    let root = project_identity(folder).ok()?;
    let mapping = map(&root);
    let shown = mapping.path().to_path_buf().into_os_string().into_string().ok()?;

    let Some(guards) = data_dir.and_then(|data_dir| super::read_listed(data_dir).ok()) else {
        return Some(shown);
    };

    let mut entries = guards.notes.on_repos.iter().chain(&guards.scripts.on_repos);
    let listed = entries.clone().any(|entry| entry == &shown);

    if listed {
        return Some(shown);
    }

    let entry = entries.find(|entry| Allowed::of([Path::new(entry.as_str())]).allows(&mapping));

    Some(entry.cloned().unwrap_or(shown))
}

/// The saving root of `folder`: its project root ([`project_identity`]) mapped by [`saving_root`].
/// `None` for a folder that is gone, not a folder, or a path that is not text.
pub fn saving_root_of_folder(folder: &Path) -> Option<String> {
    let root = project_identity(folder).ok()?;

    saving_root(&root).into_os_string().into_string().ok()
}

/// The canonical folder `root/.git` names, when that is a file of one line `gitdir: X`.
fn git_file_dir(root: &Path) -> Option<PathBuf> {
    let marker = root.join(".git");

    if !std::fs::symlink_metadata(&marker).ok()?.is_file() {
        return None;
    }

    let line = one_line(&marker)?;
    let git_dir = root.join(line.strip_prefix(GITDIR_PREFIX)?.trim_start());

    dunce::canonicalize(git_dir).ok()
}

/// The common git folder (canonical) of the verified linked worktree at `root`.
fn linked_common(root: &Path) -> Option<PathBuf> {
    let marker = root.join(".git");
    let git_dir = git_file_dir(root)?;

    let common = git_dir.join(one_line(&git_dir.join("commondir"))?);
    let common = dunce::canonicalize(common).ok()?;

    let back_link = git_dir.join(one_line(&git_dir.join("gitdir"))?);
    let back_link = dunce::canonicalize(back_link).ok()?;

    if back_link != dunce::canonicalize(&marker).ok()? {
        return None;
    }

    // The admin folder must be one of the common folder's own worktree folders.
    if git_dir.parent()? != common.join("worktrees") {
        return None;
    }

    Some(common)
}

/// The only line of the small file `path`, without its line ending; `None` when it cannot be read,
/// is too large, empty or holds more than one line.
fn one_line(path: &Path) -> Option<String> {
    let text = SafeReader::read_to_string(path, MAX_LINK_BYTES).ok()?;
    let line = text.trim_end_matches(['\r', '\n']);

    if line.is_empty() || line.contains(['\r', '\n']) {
        return None;
    }

    Some(line.to_owned())
}
