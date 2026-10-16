//! Directory scans for themes, extension modules, skills and Markdown definitions, plus display
//! helpers.
//!
//! Scans never recurse beyond what the harness itself discovers: harness homes hold tens of
//! thousands of files (sessions, caches), so every listing is capped. The few recursive scans go
//! through [`walk_tree`], which bounds depth and total entries and steps around symlink loops;
//! [`walk_tree_budgeted`] shares one entry budget between many walks. The extension-module,
//! prompt and theme scans of one settings layer share one [`ScanBudget`] for directory entries,
//! path resolutions and file reads ([`PerLayer`] keeps the project's apart from the user's), and
//! every budget records whether a cap cut a listing short, for the view's [`Truncation`] notice.

use std::collections::HashSet;
use std::hash::Hash;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::config_files::{read_json, string_list};
use crate::env::Env;
use crate::model::{ExtensionModuleInfo, ExtensionScope, ThemeInfo};

/// Maximum entries read from one directory.
const MAX_DIR_ENTRIES: usize = 2000;

/// Maximum entries one [`walk_tree`] reads in total, so a crafted tree cannot stall a view.
pub const MAX_WALK_ENTRIES: usize = 10_000;

/// Maximum distinct entries read from one manifest list (a package's `extensions`, `skills`, ...);
/// packages declare a handful.
pub const MAX_DECLARED_ENTRIES: usize = 64;

/// Steps (directory entries read, paths resolved, files read) the scans sharing one
/// [`ScanBudget::per_view`] take in total, so that many listed paths, links or aliases cannot
/// stall a view.
const MAX_VIEW_SCAN_STEPS: usize = 4 * MAX_WALK_ENTRIES;

/// Bytes of `package.json` manifests and Markdown files those scans read in total.
const MAX_VIEW_READ_BYTES: u64 = 8 * 1024 * 1024;

/// Largest `<sub>/package.json` read from an extensions directory; real ones are a few KiB.
const MAX_MODULE_MANIFEST_BYTES: u64 = 256 * 1024;

/// What one file read costs at least, so that many tiny files are bounded too.
const MIN_READ_COST: u64 = 4 * 1024;

mod budget;

pub use budget::{PerLayer, ScanBudget, Truncation, canonical_inside, distinct_targets};

/// One directory entry (symlinks are resolved for the type checks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// File name.
    pub name: String,
    /// Full path.
    pub path: PathBuf,
    /// True for directories (or links to them).
    pub is_dir: bool,
    /// True for regular files (or links to them).
    pub is_file: bool,
}

/// The entries of `dir` sorted by name; empty when `dir` is missing or unreadable. Names that are
/// not valid UTF-8 are skipped.
pub fn list_dir(dir: &Path) -> Vec<Entry> {
    list_dir_up_to(dir, MAX_DIR_ENTRIES).entries
}

/// What [`list_dir_up_to`] read.
#[derive(Debug, Default)]
struct Listing {
    entries: Vec<Entry>,
    /// Entries read, skipped ones included, so a budget is charged for every entry the
    /// filesystem returned.
    read_count: usize,
    /// True when the directory holds entries beyond the limit.
    more: bool,
}

/// [`list_dir`] reading at most `limit` entries (`0` only checks whether `dir` has any).
fn list_dir_up_to(dir: &Path, limit: usize) -> Listing {
    let Ok(mut read) = std::fs::read_dir(dir) else {
        return Listing::default();
    };

    let mut read_count = 0;
    let mut entries: Vec<Entry> = read
        .by_ref()
        .take(limit)
        .inspect(|_| read_count += 1)
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let path = entry.path();
            let metadata = std::fs::metadata(&path).ok();

            Some(Entry {
                name,
                is_dir: metadata.as_ref().is_some_and(|metadata| metadata.is_dir()),
                is_file: metadata.as_ref().is_some_and(|metadata| metadata.is_file()),
                path,
            })
        })
        .collect();

    let more = read.next().is_some();

    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Listing {
        entries,
        read_count,
        more,
    }
}

/// Walks `root` depth first, up to `max_depth` levels (`1` = the entries of `root` only), calling
/// `visit` on every entry in [`list_dir`] order. `visit` returns true to descend into a directory
/// entry.
///
/// Symlinked files and directories are followed, except a directory that resolves to one already
/// open on the current path (a symlink loop such as `link -> .`), which is not entered again. The
/// whole walk reads at most [`MAX_WALK_ENTRIES`] entries.
pub fn walk_tree(root: &Path, max_depth: usize, visit: &mut dyn FnMut(&Entry) -> bool) {
    walk_tree_budgeted(root, max_depth, &mut WalkBudget::new(MAX_WALK_ENTRIES), visit);
}

/// The entries a group of walks may still read, so that many roots (e.g. every path a settings
/// file lists) cannot multiply [`MAX_WALK_ENTRIES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkBudget {
    entries_left: usize,
    truncated: bool,
}

impl WalkBudget {
    /// A budget of `entries` directory entries.
    pub const fn new(entries: usize) -> Self {
        Self {
            entries_left: entries,
            truncated: false,
        }
    }

    /// True once a walk skipped entries because this budget, [`MAX_WALK_ENTRIES`] or the
    /// per-directory cap ran out (the depth limit does not count).
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    /// Records that a caller dropped something on its own cap (a settings list's).
    pub const fn mark_truncated(&mut self) {
        self.truncated = true;
    }
}

/// [`walk_tree`] charging every entry it reads to `budget`; one walk still reads at most
/// [`MAX_WALK_ENTRIES`].
pub fn walk_tree_budgeted(
    root: &Path,
    max_depth: usize,
    budget: &mut WalkBudget,
    visit: &mut dyn FnMut(&Entry) -> bool,
) {
    let allowance = budget.entries_left.min(MAX_WALK_ENTRIES);
    let mut walk = Walk {
        visit,
        open: Vec::new(),
        entries_left: allowance,
        truncated: false,
    };

    walk.enter(root, max_depth);
    budget.entries_left -= allowance - walk.entries_left;
    budget.truncated |= walk.truncated;
}

/// The first `limit` distinct items, in order (later repeats are dropped).
pub fn distinct<T: Hash + Eq + Clone>(items: impl IntoIterator<Item = T>, limit: usize) -> Vec<T> {
    distinct_within(items, limit).0
}

/// [`distinct`], with true when more distinct items followed the first `limit`.
pub fn distinct_within<T: Hash + Eq + Clone>(items: impl IntoIterator<Item = T>, limit: usize) -> (Vec<T>, bool) {
    let mut seen = HashSet::new();
    let mut kept = Vec::new();

    for item in items {
        if !seen.insert(item.clone()) {
            continue;
        }

        if kept.len() == limit {
            return (kept, true);
        }

        kept.push(item);
    }

    (kept, false)
}

/// The state of one [`walk_tree`].
struct Walk<'a> {
    visit: &'a mut dyn FnMut(&Entry) -> bool,
    /// Resolved paths of the directories from the root down to the current one.
    open: Vec<PathBuf>,
    entries_left: usize,
    truncated: bool,
}

impl Walk<'_> {
    fn enter(&mut self, dir: &Path, depth_left: usize) {
        if depth_left == 0 {
            return;
        }

        let resolved = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        if self.open.contains(&resolved) {
            return;
        }

        if self.entries_left == 0 {
            self.truncated |= list_dir_up_to(dir, 0).more;
            return;
        }

        let listing = list_dir_up_to(dir, self.entries_left.min(MAX_DIR_ENTRIES));
        self.entries_left -= listing.read_count;
        self.truncated |= listing.more;
        self.open.push(resolved);

        for entry in listing.entries {
            if (self.visit)(&entry) && entry.is_dir {
                self.enter(&entry.path, depth_left - 1);
            }
        }

        self.open.pop();
    }
}

/// `path` with the home directory shown as `~` (for human-readable origins).
pub fn tilde(env: &Env, path: &Path) -> String {
    match path.strip_prefix(&env.home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Ok(rest) => format!("~/{}", rest.to_string_lossy().replace('\\', "/")),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

/// `dir` relative to `project` when inside it, else [`tilde`].
pub fn origin_label(env: &Env, project: Option<&Path>, dir: &Path) -> String {
    if let Some(rest) = project.and_then(|project| dir.strip_prefix(project).ok()) {
        return rest.to_string_lossy().replace('\\', "/");
    }

    tilde(env, dir)
}

/// Built-in themes, flagged active when `is_active(name)`.
pub fn builtin_themes(names: &[&str], is_active: &dyn Fn(&str) -> bool) -> Vec<ThemeInfo> {
    names
        .iter()
        .map(|name| ThemeInfo {
            name: (*name).to_owned(),
            scope: ExtensionScope::Builtin,
            path: None,
            active: is_active(name),
        })
        .collect()
}

/// Custom themes: the `*.json` files directly inside `dir` (contents are not read).
pub fn themes_in_dir(dir: &Path, scope: ExtensionScope, is_active: &dyn Fn(&str) -> bool) -> Vec<ThemeInfo> {
    themes_in_dir_budgeted(dir, scope, is_active, &mut ScanBudget::per_view())
}

/// [`themes_in_dir`] listing `dir` within `budget`.
pub fn themes_in_dir_budgeted(
    dir: &Path,
    scope: ExtensionScope,
    is_active: &dyn Fn(&str) -> bool,
    budget: &mut ScanBudget,
) -> Vec<ThemeInfo> {
    budget
        .list_dir(dir)
        .into_iter()
        .filter(|entry| entry.is_file)
        .filter_map(|entry| {
            let name = entry.name.strip_suffix(".json")?.to_owned();

            Some(ThemeInfo {
                active: is_active(&name),
                name,
                scope,
                path: Some(entry.path.to_string_lossy().into_owned()),
            })
        })
        .collect()
}

/// Extension module entry points in `dir`, following the conventions shared by OMP and PI:
/// direct `*.ts`/`*.js` files, `<sub>/package.json` declaring `omp.extensions`/`pi.extensions`,
/// otherwise `<sub>/index.ts` or `<sub>/index.js`. One level deep only.
///
/// The scan runs within `budget`. Subdirectories resolving to the same directory (links to one
/// target) count once, the first by name; one that cannot be resolved (or once the steps are
/// spent) is skipped. A subdirectory whose manifest is not read (read bytes spent, larger than
/// 256 KiB, unreadable) falls back to its `index.*` file, as for an invalid manifest.
pub fn extension_module_paths(dir: &Path, budget: &mut ScanBudget) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut seen_dirs = HashSet::new();

    for entry in budget.list_dir(dir) {
        if entry.is_file && is_script(&entry.name) {
            found.push(entry.path);
            continue;
        }

        if !entry.is_dir || entry.name.starts_with('.') {
            continue;
        }

        let Some(resolved) = budget.canonicalize(&entry.path) else {
            continue;
        };
        if !seen_dirs.insert(resolved.clone()) {
            continue;
        }

        let declared = declared_extensions(&resolved, budget);
        if !declared.is_empty() {
            found.extend(declared);
            continue;
        }

        let index = ["index.ts", "index.js"]
            .iter()
            .map(|name| entry.path.join(name))
            .find(|path| path.is_file());
        found.extend(index);
    }

    found
}

/// A module named after its file stem, or its directory for `index.*` entry points.
pub fn extension_module(path: &Path, scope: ExtensionScope, enabled: bool) -> ExtensionModuleInfo {
    ExtensionModuleInfo {
        name: extension_name(path),
        path: path.to_string_lossy().into_owned(),
        scope,
        enabled,
    }
}

/// The display name OMP and PI derive from a module path.
pub fn extension_name(path: &Path) -> String {
    let file_name = path.file_name().map(|name| name.to_string_lossy().into_owned());
    let stem = path.file_stem().map(|stem| stem.to_string_lossy().into_owned());
    let is_index = matches!(file_name.as_deref(), Some("index.ts" | "index.js"));

    let parent = path.parent().and_then(Path::file_name);
    if is_index && let Some(parent) = parent {
        return parent.to_string_lossy().into_owned();
    }

    stem.or(file_name).unwrap_or_default()
}

/// The `omp` (or else `pi`) manifest of `<dir>/package.json`, if it is an object.
pub fn package_manifest(dir: &Path) -> Option<Value> {
    harness_manifest(read_json(&dir.join("package.json")).value()?)
}

/// The `omp` (or else `pi`) object of a parsed `package.json`.
fn harness_manifest(package: &Value) -> Option<Value> {
    ["omp", "pi"]
        .iter()
        .find_map(|key| package.get(*key).filter(|manifest| manifest.is_object()).cloned())
}

/// Entry points a package declares, resolved inside the canonical package directory (escapes are
/// dropped). Repeated entries count once, and at most [`MAX_DECLARED_ENTRIES`] are resolved, all
/// within `budget`.
fn declared_extensions(package_dir: &Path, budget: &mut ScanBudget) -> Vec<PathBuf> {
    let Some(manifest) = budget.read_manifest(package_dir) else {
        return Vec::new();
    };

    let (declared, capped) = distinct_within(string_list(manifest.get("extensions")), MAX_DECLARED_ENTRIES);
    if capped {
        budget.mark_truncated();
    }

    let resolved: Vec<PathBuf> = declared
        .iter()
        .filter_map(|declared| budget.canonical_inside(package_dir, Path::new(declared)))
        .collect();

    distinct(resolved, MAX_DECLARED_ENTRIES)
        .into_iter()
        .filter_map(|path| {
            if path.is_file() {
                return Some(path);
            }

            ["index.ts", "index.js"]
                .iter()
                .map(|name| path.join(name))
                .find(|index| index.is_file())
        })
        .collect()
}

fn is_script(name: &str) -> bool {
    name.ends_with(".ts") || name.ends_with(".js")
}

#[cfg(test)]
mod tests;
