//! The budgets of one view's resource scans (see [`ScanBudget`], [`PerLayer`]), the path checks
//! that spend them, and the notice shown when one ran out ([`Truncation`]).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::super::config_files::parse_text;
use super::super::frontmatter::{MAX_MARKDOWN_BYTES, parse_frontmatter};
use super::{
    Entry, MAX_DIR_ENTRIES, MAX_MODULE_MANIFEST_BYTES, MAX_VIEW_READ_BYTES, MAX_VIEW_SCAN_STEPS, MIN_READ_COST,
    WalkBudget, distinct_within, harness_manifest, list_dir_up_to,
};
use crate::error::BusinessError;
use crate::model::{ConfigFormat, ExtensionScope, Notice, NoticeLevel};
use crate::security::SafeReader;

/// What a group of resource scans (one layer's extension modules, prompts and themes) may still
/// do: directory entries read and paths resolved (steps), and bytes read from manifests and
/// Markdown files. Once the steps are spent, listings come back empty and nothing more is
/// resolved or read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanBudget {
    pub(super) steps_left: usize,
    pub(super) bytes_left: u64,
    truncated: bool,
}

impl ScanBudget {
    /// A budget of `steps` steps and `bytes` read bytes.
    pub const fn new(steps: usize, bytes: u64) -> Self {
        Self {
            steps_left: steps,
            bytes_left: bytes,
            truncated: false,
        }
    }

    /// The budget of one settings layer of a view (or of one conventional extensions directory).
    pub const fn per_view() -> Self {
        Self::new(MAX_VIEW_SCAN_STEPS, MAX_VIEW_READ_BYTES)
    }

    /// True once a scan skipped something because this budget or the per-directory or
    /// per-list cap ran out.
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    /// Records that a caller dropped something on its own cap (a settings list's).
    pub const fn mark_truncated(&mut self) {
        self.truncated = true;
    }

    /// The entries of `dir` like [`list_dir`](super::list_dir), one step for every entry read.
    pub fn list_dir(&mut self, dir: &Path) -> Vec<Entry> {
        let listing = list_dir_up_to(dir, self.steps_left.min(MAX_DIR_ENTRIES));
        self.steps_left -= listing.read_count;
        self.truncated |= listing.more;

        listing.entries
    }

    /// `path` resolved through `..` and symlinks, for one step; `None` when it does not resolve or
    /// the budget is spent.
    pub fn canonicalize(&mut self, path: &Path) -> Option<PathBuf> {
        if !self.take_step(path) {
            return None;
        }

        dunce::canonicalize(path).ok()
    }

    /// [`canonical_inside`] for one step; `None` once the budget is spent.
    pub fn canonical_inside(&mut self, root: &Path, candidate: &Path) -> Option<PathBuf> {
        if !self.take_step(&root.join(candidate)) {
            return None;
        }

        canonical_inside(root, candidate)
    }

    /// True when `path` resolves inside the canonical directory `root` (one step).
    pub fn is_inside(&mut self, root: &Path, path: &Path) -> bool {
        self.canonical_inside(root, path).is_some()
    }

    /// The frontmatter of the Markdown file at `path` (see [`super::read_frontmatter`]), read
    /// within the budget.
    pub fn read_frontmatter(&mut self, path: &Path) -> Option<Value> {
        let text = self.read(path, MAX_MARKDOWN_BYTES)?;
        parse_frontmatter(&text)
    }

    /// The `omp`/`pi` manifest of `<package_dir>/package.json`, read within the budget.
    pub(super) fn read_manifest(&mut self, package_dir: &Path) -> Option<Value> {
        let text = self.read(&package_dir.join("package.json"), MAX_MODULE_MANIFEST_BYTES)?;
        let package = parse_text(&text, ConfigFormat::Json).ok()?;

        harness_manifest(&package)
    }

    /// Reads `path` (at most `max_bytes`) for one step plus the bytes read, at least
    /// [`MIN_READ_COST`]. A missing file costs only the step. One rejected before any byte is read
    /// (larger than the allowance, not a regular file, denied) costs [`MIN_READ_COST`], one read
    /// and then rejected (not UTF-8) its size.
    fn read(&mut self, path: &Path, max_bytes: u64) -> Option<String> {
        let allowance = self.bytes_left.min(max_bytes);
        if allowance == 0 {
            self.truncated |= path.is_file();
            return None;
        }

        if !self.take_step(path) {
            return None;
        }

        let size = match std::fs::metadata(path) {
            Ok(metadata) if metadata.is_file() => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            _ => {
                self.charge(MIN_READ_COST);
                return None;
            }
        };

        if size > allowance {
            // Within `max_bytes`, only the bytes left were short.
            self.truncated |= size <= max_bytes;
            self.charge(MIN_READ_COST);
            return None;
        }

        let read = SafeReader::read_to_string(path, allowance);
        let cost = match &read {
            Ok(text) => text.len() as u64,
            Err(BusinessError::NotFound(_)) => return None,
            Err(BusinessError::Forbidden(_)) => MIN_READ_COST,
            // Read, then rejected: not UTF-8, or the file grew past the allowance meanwhile.
            Err(_) => std::fs::metadata(path).map_or(size, |metadata| metadata.len().max(size)),
        };
        self.charge(cost.clamp(MIN_READ_COST, allowance.max(MIN_READ_COST)));

        read.ok()
    }

    /// Spends `bytes` of the read allowance.
    fn charge(&mut self, bytes: u64) {
        self.bytes_left = self.bytes_left.saturating_sub(bytes);
    }

    /// Takes one step for `path`; once the steps are spent, an existing `path` marks the budget
    /// truncated.
    fn take_step(&mut self, path: &Path) -> bool {
        if self.steps_left == 0 {
            self.truncated |= path.exists();
            return false;
        }

        self.steps_left -= 1;
        true
    }
}

/// One budget for the project layer and one for everything else (the user layer, its packages,
/// built-in locations), so that a project's settings cannot spend what the user's resources need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerLayer<T> {
    /// Charged for resources the project's settings list (and project packages).
    pub project: T,
    /// Charged for everything else.
    pub user: T,
}

impl<T: Copy> PerLayer<T> {
    /// `budget` for each layer.
    pub const fn new(budget: T) -> Self {
        Self {
            project: budget,
            user: budget,
        }
    }

    /// The budget resources of `scope` are charged to.
    pub fn get(&mut self, scope: ExtensionScope) -> &mut T {
        match scope {
            ExtensionScope::Project => &mut self.project,
            _ => &mut self.user,
        }
    }
}

/// Shown when the project's scans hit a cap: an untrusted project may be hiding something.
const PROJECT_TRUNCATED_NOTICE: &str = "Some extensions were not listed because this project lists more than the app \
     reads. Check its settings before trusting it.";

/// Shown when the user's own scans hit a cap.
const USER_TRUNCATED_NOTICE: &str = "Some extensions were not listed because your setup lists more than the app reads.";

/// Which layers had a listing cut short by a budget or cap, for the view's notices.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Truncation {
    project: bool,
    user: bool,
}

impl Truncation {
    /// Records `truncated` for the layer of `scope` (project, else user).
    pub fn note(&mut self, scope: ExtensionScope, truncated: bool) {
        match scope {
            ExtensionScope::Project => self.project |= truncated,
            _ => self.user |= truncated,
        }
    }

    /// Records the scan budgets of both layers.
    pub fn note_scans(&mut self, budgets: &PerLayer<ScanBudget>) {
        self.project |= budgets.project.truncated();
        self.user |= budgets.user.truncated();
    }

    /// Records the walk budgets of both layers.
    pub fn note_walks(&mut self, budgets: &PerLayer<WalkBudget>) {
        self.project |= budgets.project.truncated();
        self.user |= budgets.user.truncated();
    }

    /// Records everything `other` recorded.
    pub fn merge(&mut self, other: Self) {
        self.project |= other.project;
        self.user |= other.user;
    }

    /// One notice per layer that was cut short: a warning for the project, information for the
    /// user.
    pub fn notices(&self) -> Vec<Notice> {
        let mut notices = Vec::new();

        if self.project {
            notices.push(Notice {
                level: NoticeLevel::Warning,
                message: PROJECT_TRUNCATED_NOTICE.to_owned(),
            });
        }

        if self.user {
            notices.push(Notice {
                level: NoticeLevel::Info,
                message: USER_TRUNCATED_NOTICE.to_owned(),
            });
        }

        notices
    }
}

/// `candidate` (relative paths taken from `root`) resolved through `..` and symlinks, when it
/// exists and lies inside `root`. `root` must already be canonical (an installed package's
/// directory, a resolved subdirectory), so it is not resolved again for every candidate.
pub fn canonical_inside(root: &Path, candidate: &Path) -> Option<PathBuf> {
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };

    dunce::canonicalize(joined)
        .ok()
        .filter(|resolved| resolved.starts_with(root))
}

/// The first `limit` distinct `paths` (as [`distinct`](super::distinct)), with aliases of one
/// target (`hub`, `hub/../hub`, links to it) counted once, the first kept as written. Only those
/// `limit` paths are resolved, each for one step; one that does not resolve is compared as
/// written. Paths beyond `limit` mark the budget truncated.
pub fn distinct_targets(
    paths: impl IntoIterator<Item = PathBuf>,
    limit: usize,
    budget: &mut ScanBudget,
) -> Vec<PathBuf> {
    let mut targets = HashSet::new();

    let (kept, capped) = distinct_within(paths, limit);
    if capped {
        budget.mark_truncated();
    }

    kept.into_iter()
        .filter(|path| {
            let target = budget.canonicalize(path).unwrap_or_else(|| path.clone());
            targets.insert(target)
        })
        .collect()
}
