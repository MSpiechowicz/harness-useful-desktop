//! `project_files`: the files and folders of a project that match what the user typed after `@` in
//! the composer, best first.
//!
//! The list comes from `git ls-files` (untracked files that `.gitignore` does not hide, and tracked
//! ones) in a work tree, and from a bounded walk of the folder anywhere else (see [`walk`]). A file is
//! marked `changed` when `git status` lists it (a tracked file it lists as deleted from the working
//! tree is not listed at all). Folders are derived from the files below them. The
//! matching and ranking happen here: the webview only receives the few best matches.
//!
//! # Deny-list
//!
//! Every path is checked by the lexical deny-list ([`crate::security::is_denied`]) when the list is
//! built, so a denied file or folder is not in it at all. The result paths are checked again with
//! the stronger [`super::is_denied_in`] (it resolves symbolic links, so it is expensive), walking
//! down the ranked matches until the wanted number is filled or [`probe_limit`] matches were
//! looked at. A path that leads through a symbolic link is offered only when its real path lies in
//! the project and is offered itself, so a link to a key never shows under a harmless name. Paths with
//! a control or invisible character are never listed: a picked path goes straight into a prompt.
//!
//! # Cost
//!
//! Building the list costs two `git ls-files` (untracked, tracked) and one `git status` (or a walk), see
//! [`ProjectFileCache`] for how often. A search scores every entry once and sorts only the best of
//! them ([`rank`]).

mod cache;
mod entries;
mod rank;
mod score;
mod walk;

pub use cache::{MAX_CACHED_PROJECTS, PROJECT_FILES_TTL, ProjectFileCache};

use std::fs;
use std::path::{Component, Path};
use std::time::Duration;

use entries::{Changed, Entry, entries, is_offered};
use rank::{CHANGED_BOOST, Match, Ranked};
use score::Matcher;

use super::cli::GitCli;
use super::{is_denied_in, status};
use crate::env::Env;
use crate::error::Result;
use crate::model::{GitFileStatus, MAX_PROJECT_FILE_QUERY_BYTES, MAX_PROJECT_FILES, ProjectFile};
use crate::process::{ExecOutput, ProcessRunner};

/// The disk probes (a deny check and a `lstat`) a search makes per wanted result, at most.
const PROBES_PER_RESULT: usize = 40;

/// The fewest probes a search makes, however few results it wants.
const MIN_PROBES: usize = 400;

/// The searchable files and folders of one project, as of the moment it was built.
pub struct ProjectFileIndex {
    entries: Vec<Entry>,
}

impl ProjectFileIndex {
    /// Lists the files of the canonical project directory `project`. Reads the disk and runs git:
    /// call it from a blocking task. A folder that is no git work tree (or has no `git` to ask) is
    /// walked instead. An empty folder gives an empty list.
    pub async fn build(env: &Env, runner: &dyn ProcessRunner, project: &Path) -> Result<Self> {
        let git = match GitCli::new(env, runner, project) {
            Ok(git) => git,
            Err(_) => return Ok(Self::walked(project)),
        };

        if git.prefix().await?.is_none() {
            return Ok(Self::walked(project));
        }

        Self::listed_by_git(&git).await
    }

    fn walked(project: &Path) -> Self {
        let files = walk::walk(project);

        Self {
            entries: entries(files.iter().map(String::as_str), &Changed::default()),
        }
    }

    /// Untracked files that are not ignored, then tracked files. Each is its own `git ls-files` run
    /// with its own [`GIT_OUTPUT_CAP`], so a huge tracked list cannot crowd the files being worked on
    /// out of the output. A list that git cut is used up to its last whole name.
    async fn listed_by_git(git: &GitCli<'_>) -> Result<Self> {
        let (untracked, tracked, state) = tokio::join!(
            git.run_ok(["ls-files", "-z", "--others", "--exclude-standard"]),
            git.run_ok(["ls-files", "-z", "--cached"]),
            status::git_status(git),
        );
        let (untracked, tracked) = (untracked?, tracked?);

        let mut changed = Changed::default();
        if let Ok(state) = state {
            for entry in &state.entries {
                if entry.unstaged == Some(GitFileStatus::Deleted) {
                    changed.add_deleted(&entry.path);
                } else {
                    changed.add(&entry.path);
                }
            }
        }

        let names = whole_names(&untracked).chain(whole_names(&tracked));

        Ok(Self {
            entries: entries(names, &changed),
        })
    }

    /// The matches of `query`, at most `limit` (1 to 50), best first. `project` is the directory the
    /// list was built for: a match is dropped when it is gone from it, or when the stronger deny-list
    /// check refuses it. An over-long query matches nothing.
    ///
    /// The order is a score (a match in the file name beats one in a folder, consecutive characters
    /// and the start of a word score higher) plus a boost for a changed file, then the shorter path,
    /// then the alphabetical one. Without a query it is the changed files, then the shallower paths,
    /// then the alphabetical ones.
    pub fn search(&self, project: &Path, query: &str, limit: usize) -> Vec<ProjectFile> {
        let limit = limit.clamp(1, MAX_PROJECT_FILES);

        if query.len() > MAX_PROJECT_FILE_QUERY_BYTES {
            return Vec::new();
        }

        let mut matcher = Matcher::new(query);

        let matches: Vec<Match> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let score = match matcher.as_mut() {
                    Some(matcher) => matcher.score(&entry.path)?,
                    None => 0,
                };
                let boost = if entry.changed { CHANGED_BOOST } else { 0 };

                Some(Match {
                    score: score + boost,
                    index,
                })
            })
            .collect();

        // A list can hold thousands of files that are gone (a sparse checkout, a hostile index):
        // looking at each would take seconds, so a search gives up and returns what it found.
        Ranked::new(&self.entries, matches, matcher.is_none(), limit)
            .take(probe_limit(limit))
            .filter_map(|entry| confirmed(project, entry))
            .take(limit)
            .collect()
    }

    /// Number of files and folders in the list.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the project has no listable file.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The most matches a search for `limit` results looks at on the disk.
fn probe_limit(limit: usize) -> usize {
    limit.saturating_mul(PROBES_PER_RESULT).max(MIN_PROBES)
}

/// The names of one `ls-files -z` output, without the last one when git cut the output there.
fn whole_names(listing: &ExecOutput) -> impl Iterator<Item = &str> {
    let mut names: Vec<&str> = listing.stdout.split('\0').collect();
    if listing.truncated {
        names.pop();
    }

    names.into_iter()
}

/// The entry as a result when it is still there and the full deny-list check lets it through.
fn confirmed(project: &Path, entry: &Entry) -> Option<ProjectFile> {
    if is_denied_in(project, &entry.path) {
        return None;
    }

    // The disk can have changed since the list was built (a file deleted, a submodule's folder).
    let full = project.join(&*entry.path);
    let on_disk = fs::symlink_metadata(&full).ok()?;

    if !leads_to_offered(project, &entry.path, &full) {
        return None;
    }

    Some(ProjectFile {
        path: entry.path.to_string(),
        is_dir: entry.is_dir || on_disk.is_dir(),
        changed: entry.changed,
    })
}

/// True when `full`, the path of the project-relative `relative`, really lies in the project at a
/// path that is offered as well. The deny check meets the name of a link, not what it points at: a
/// tracked link called `notes.md` to `~/.ssh/id_rsa` (or a folder link in the middle of the path) would
/// pass it. A target that cannot be resolved or is outside the project is refused.
fn leads_to_offered(project: &Path, relative: &str, full: &Path) -> bool {
    let (Ok(real), Ok(root)) = (dunce::canonicalize(full), dunce::canonicalize(project)) else {
        return false;
    };

    let Ok(inside) = real.strip_prefix(&root) else {
        return false;
    };

    let names: Option<Vec<&str>> = inside
        .components()
        .map(|component| match component {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();

    let Some(names) = names else {
        return false;
    };

    let real_relative = names.join("/");

    // The path as listed is offered already: only another real path needs the check.
    real_relative == relative || (!real_relative.is_empty() && is_offered(&real_relative))
}

/// `project_files` for the canonical project directory `project`, listing it afresh: the
/// [`ProjectFileCache`] is what a caller that searches again and again wants. Blocks like
/// [`ProjectFileIndex::build`].
pub async fn project_files(
    env: &Env,
    runner: &dyn ProcessRunner,
    project: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<ProjectFile>> {
    // A list that expires at once is never reused: every call builds its own.
    ProjectFileCache::with_ttl(Duration::ZERO)
        .search(env, runner, project, query, limit)
        .await
}
