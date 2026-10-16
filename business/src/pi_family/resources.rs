//! Directory scans for themes, extension modules, skills and Markdown definitions, plus display
//! helpers.
//!
//! Scans never recurse beyond what the harness itself discovers: harness homes hold tens of
//! thousands of files (sessions, caches), so every listing is capped. The few recursive scans go
//! through [`walk_tree`], which bounds depth and total entries and steps around symlink loops.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::config_files::{read_json, string_list};
use crate::env::Env;
use crate::model::{ExtensionModuleInfo, ExtensionScope, ThemeInfo};
use crate::security::canonical_within;

/// Maximum entries read from one directory.
const MAX_DIR_ENTRIES: usize = 2000;

/// Maximum entries one [`walk_tree`] reads in total, so a crafted tree cannot stall a view.
const MAX_WALK_ENTRIES: usize = 10_000;

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
    list_dir_up_to(dir, MAX_DIR_ENTRIES).0
}

/// [`list_dir`] reading at most `limit` entries, with the number of entries read (skipped ones
/// included, so a budget is charged for every entry the filesystem returned).
fn list_dir_up_to(dir: &Path, limit: usize) -> (Vec<Entry>, usize) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return (Vec::new(), 0);
    };

    let mut read_count = 0;
    let mut entries: Vec<Entry> = read
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

    entries.sort_by(|left, right| left.name.cmp(&right.name));
    (entries, read_count)
}

/// Walks `root` depth first, up to `max_depth` levels (`1` = the entries of `root` only), calling
/// `visit` on every entry in [`list_dir`] order. `visit` returns true to descend into a directory
/// entry.
///
/// Symlinked files and directories are followed, except a directory that resolves to one already
/// open on the current path (a symlink loop such as `link -> .`), which is not entered again. The
/// whole walk reads at most [`MAX_WALK_ENTRIES`] entries.
pub fn walk_tree(root: &Path, max_depth: usize, visit: &mut dyn FnMut(&Entry) -> bool) {
    let mut walk = Walk {
        visit,
        open: Vec::new(),
        entries_left: MAX_WALK_ENTRIES,
    };

    walk.enter(root, max_depth);
}

/// The state of one [`walk_tree`].
struct Walk<'a> {
    visit: &'a mut dyn FnMut(&Entry) -> bool,
    /// Resolved paths of the directories from the root down to the current one.
    open: Vec<PathBuf>,
    entries_left: usize,
}

impl Walk<'_> {
    fn enter(&mut self, dir: &Path, depth_left: usize) {
        if depth_left == 0 || self.entries_left == 0 {
            return;
        }

        let resolved = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        if self.open.contains(&resolved) {
            return;
        }

        let (entries, read_count) = list_dir_up_to(dir, self.entries_left.min(MAX_DIR_ENTRIES));
        self.entries_left -= read_count;
        self.open.push(resolved);

        for entry in entries {
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
    list_dir(dir)
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
pub fn extension_module_paths(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();

    for entry in list_dir(dir) {
        if entry.is_file && is_script(&entry.name) {
            found.push(entry.path);
            continue;
        }

        if !entry.is_dir || entry.name.starts_with('.') {
            continue;
        }

        let declared = declared_extensions(&entry.path);
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
    let package = read_json(&dir.join("package.json")).value()?.clone();

    ["omp", "pi"]
        .iter()
        .find_map(|key| package.get(*key).filter(|manifest| manifest.is_object()).cloned())
}

/// Entry points a package declares, resolved inside the package directory (escapes are dropped).
fn declared_extensions(package_dir: &Path) -> Vec<PathBuf> {
    let Some(manifest) = package_manifest(package_dir) else {
        return Vec::new();
    };

    string_list(manifest.get("extensions"))
        .iter()
        .filter_map(|declared| canonical_within(package_dir, Path::new(declared)).ok())
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
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_walk_stops_after_its_entry_budget() {
        let temp = tempfile::TempDir::new().unwrap();
        let files = temp.path().join("files");
        let root = temp.path().join("root");
        std::fs::create_dir_all(&files).unwrap();
        std::fs::create_dir_all(&root).unwrap();

        for index in 0..MAX_DIR_ENTRIES {
            std::fs::write(files.join(format!("{index}.md")), "").unwrap();
        }
        // Six links to the same full directory are not a loop, so each is walked in turn.
        for name in ["a", "b", "c", "d", "e", "f"] {
            std::os::unix::fs::symlink(&files, root.join(name)).unwrap();
        }

        let mut visited = 0;
        walk_tree(&root, 2, &mut |_| {
            visited += 1;
            true
        });

        assert_eq!(visited, MAX_WALK_ENTRIES);
    }

    #[cfg(unix)]
    #[test]
    fn skipped_names_still_count_against_the_budget() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let temp = tempfile::TempDir::new().unwrap();
        let leaf = temp.path().join("leaf");
        let middle = temp.path().join("middle");
        let root = temp.path().join("root");
        for dir in [&leaf, &middle, &root] {
            std::fs::create_dir_all(dir).unwrap();
        }

        // Names that are not UTF-8 are never visited, but reading them is not free.
        for index in 0..MAX_DIR_ENTRIES {
            let name = [b"\xff".as_slice(), index.to_string().as_bytes()].concat();
            std::fs::write(leaf.join(OsStr::from_bytes(&name)), "").unwrap();
        }
        for index in 0..MAX_DIR_ENTRIES {
            std::os::unix::fs::symlink(&leaf, middle.join(format!("{index}"))).unwrap();
        }
        for name in ["a", "b", "c", "d", "e"] {
            std::os::unix::fs::symlink(&middle, root.join(name)).unwrap();
        }

        let mut visited = 0;
        walk_tree(&root, 3, &mut |_| {
            visited += 1;
            true
        });

        // The root's links and one `middle` listing; the leaf reads then use up the budget.
        assert_eq!(visited, 5 + MAX_DIR_ENTRIES);
    }
}
