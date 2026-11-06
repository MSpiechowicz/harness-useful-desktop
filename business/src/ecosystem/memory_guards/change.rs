//! One change to `guards/global.json` ([`apply`]): turning a repository on or off for a kind. A
//! repository is turned on as its saving root ([`saving_root`]) with the project key it had then
//! ([`Pin`]); the ones already listed are never resolved again, since the folders around them can
//! change (a `git init` in a parent turns `P/L` into `P`) and a change of something else must not
//! widen what the user turned on. Turning a listed repository on again replaces its pin, and a pin
//! never outlives its entry.

use std::path::{Path, PathBuf};

use super::saving_root::{is_git_folder, names_itself, repo_is_on, saving_root, separate_git_dir};
use super::{MAX_ON_REPOS, MAX_REPO_CHARS, Pin, read_listed, write_global};
use crate::error::{BusinessError, Result};
use crate::hooks::project_identity;
use crate::model::{GlobalKindGuard, MemoryGuardKind, MemoryGuards, MemoryGuardsPatch};

/// [`update_global`](super::update_global) without the lock: the caller holds it.
pub(super) fn apply(data_dir: &Path, patch: &MemoryGuardsPatch, added: Option<Pin>) -> Result<MemoryGuards> {
    let mut guards = read_listed(data_dir)?;
    let guard = match patch.kind {
        MemoryGuardKind::Notes => &mut guards.notes,
        MemoryGuardKind::Scripts => &mut guards.scripts,
    };

    if let Some(paused) = patch.paused {
        guard.paused = paused;
    }

    if let Some(every_project) = patch.every_project {
        guard.every_project = every_project;
    }

    if let Some(repo) = &patch.remove_repo {
        remove(&mut guard.on_repos, repo);
    }

    if let Some(repo) = &patch.add_repo {
        let Some(Pin(key)) = added else {
            return Err(BusinessError::InvalidInput(
                "a repository is turned on with the project key it has now".to_owned(),
            ));
        };

        let entry = canonical_repo(repo)?;
        guard.repo_keys.insert(entry.clone(), key);
        guard.on_repos.push(entry);
    }

    write_global(data_dir, &guards)
}

/// `guard` as it is written: every repository checked, none resolved again, without duplicates,
/// sorted, each with its pin and no pin left for an entry that is not listed.
pub(super) fn listed_kind(guard: &GlobalKindGuard) -> Result<GlobalKindGuard> {
    if guard.on_repos.len() > MAX_ON_REPOS {
        return Err(BusinessError::InvalidInput(format!(
            "at most {MAX_ON_REPOS} repositories can be turned on"
        )));
    }

    let mut repos = Vec::new();

    for repo in &guard.on_repos {
        check_repo(repo)?;

        if !repos.contains(repo) {
            repos.push(repo.clone());
        }
    }

    repos.sort();

    let repo_keys = guard
        .repo_keys
        .iter()
        .filter(|(entry, _)| repos.contains(entry))
        .map(|(entry, key)| (entry.clone(), key.clone()))
        .collect();

    Ok(GlobalKindGuard {
        paused: guard.paused,
        every_project: guard.every_project,
        on_repos: repos,
        repo_keys,
        dead_repos: Vec::new(),
        changed_repos: Vec::new(),
    })
}

/// Turns `repo` off in `on_repos`: the entry written as given, and every entry that turns on the
/// project `repo` is in (a worktree and its checkout, a git folder and the checkout that keeps it
/// there, and the git folder a removed checkout keeps), so one turn-off really stops saving. A
/// folder that can no longer be resolved (it became a file, say) still removes the entry written
/// as given.
///
/// An entry that names nothing ([`names_itself`]: a link in it, or a folder that is gone) is
/// removed alone: the project it would resolve to may be listed on its own, and cleaning up the
/// stale entry must not turn that off.
///
/// When `repo` is a git folder (what a worktree of a checkout with a separate git folder sends),
/// the checkouts that keep it elsewhere go too: they turn its worktrees on.
fn remove(on_repos: &mut Vec<String>, repo: &str) {
    let stale = on_repos.iter().any(|on| on == repo) && !names_itself(Path::new(repo));

    if stale {
        on_repos.retain(|on| on != repo);
        return;
    }

    let root = listed_root(Path::new(repo));
    let kept_elsewhere = root.as_deref().and_then(separate_git_dir);

    on_repos.retain(|on| {
        let same = on == repo;
        let allows = root
            .as_deref()
            .is_some_and(|root| repo_is_on(root, std::slice::from_ref(on)));
        let git_dir = kept_elsewhere
            .as_deref()
            .is_some_and(|git_dir| git_dir == Path::new(on));
        let keeps_root = root.as_deref().is_some_and(|root| keeps_git_folder(on, root));

        !same && !allows && !git_dir && !keeps_root
    });
}

/// True when `entry` is a checkout that keeps the git folder `root` elsewhere ([`separate_git_dir`]):
/// it turns on the worktrees of `root`, though `root` is not under it. Only an entry that names
/// itself counts ([`names_itself`]), like one the guards believe.
fn keeps_git_folder(entry: &str, root: &Path) -> bool {
    let entry = Path::new(entry);

    names_itself(entry) && separate_git_dir(entry).as_deref() == Some(root)
}

/// `repo` as its saving root ([`saving_root`]: the project root, a worktree as its main
/// repository); as given when the folder is gone, and canonical when it is shaped like a git
/// folder. Every branch yields a canonical path except the one for a folder that is gone: that
/// entry names nothing ([`names_itself`]) until its folder exists as itself.
fn canonical_repo(repo: &str) -> Result<String> {
    check_repo(repo)?;

    let path = Path::new(repo);

    // A git folder is stored as itself: the nearest `.git` above one inside another repository's
    // work tree would turn that repository on instead.
    let root = if is_git_folder(path) {
        dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    } else {
        match project_identity(path) {
            Ok(root) => saving_root(&root),
            Err(BusinessError::NotFound(_)) => path.to_path_buf(),
            Err(error) => return Err(error),
        }
    };

    root.into_os_string()
        .into_string()
        .map_err(|_| BusinessError::InvalidInput("a repository path is not valid text".to_owned()))
}

/// The project root `path` is in, before it is mapped to a saving root; `None` when it cannot be
/// resolved.
fn listed_root(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }

    if is_git_folder(path) {
        return Some(dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()));
    }

    project_identity(path).ok()
}

/// `invalidInput` unless `repo` is an absolute folder path of a sensible length.
fn check_repo(repo: &str) -> Result<()> {
    if Path::new(repo).is_absolute() && repo.chars().count() <= MAX_REPO_CHARS && !repo.chars().any(char::is_control) {
        return Ok(());
    }

    Err(BusinessError::InvalidInput(
        "a repository is an absolute folder path".to_owned(),
    ))
}
