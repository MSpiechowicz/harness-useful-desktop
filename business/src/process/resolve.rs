//! Harness binary discovery without a shell.
//!
//! GUI apps on macOS (and some Linux launchers) do not inherit the login shell's `PATH`, so after
//! `PATH` the well-known install directories are searched ([`extra_search_dirs`]): bun
//! (`~/.bun/bin`), `~/.local/bin`, cargo (`~/.cargo/bin`), npm (`~/.npm-global/bin`, and
//! `%APPDATA%\npm` on Windows), Homebrew (`/opt/homebrew/bin` on macOS) and `/usr/local/bin`
//! (macOS and Linux).
//! The shell is never asked for its `PATH`.

use std::path::{Path, PathBuf};

use crate::env::{Env, OsKind};
use crate::model::{BinaryOverride, HarnessId};

/// A located executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBinary {
    /// Absolute path of the executable.
    pub path: PathBuf,
    /// True for Windows `.cmd`/`.bat` wrappers (npm shims). `cmd.exe` re-parses their arguments,
    /// so callers must only pass arguments accepted by `security::is_safe_arg` to them.
    pub is_batch_script: bool,
}

impl ResolvedBinary {
    fn new(path: PathBuf) -> Self {
        let is_batch_script = is_batch_script(&path);
        Self { path, is_batch_script }
    }
}

/// True when `path` has a `.cmd` or `.bat` extension (case-insensitive).
pub fn is_batch_script(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat"))
}

/// The user's override path for `harness`, if any (the last entry wins).
pub fn find_override(overrides: &[BinaryOverride], harness: HarnessId) -> Option<&Path> {
    overrides
        .iter()
        .rev()
        .find(|entry| entry.harness == harness)
        .map(|entry| Path::new(entry.path.as_str()))
}

/// Locates executable `name`.
///
/// Order: `explicit` override (used only when it is an absolute path to an existing file;
/// otherwise discovery continues) → absolute `PATH` entries from `env` → [`extra_search_dirs`].
///
/// Relative and empty `PATH` entries (`.`, `bin`, `::`) are ignored: they would resolve against
/// whatever directory the app happens to run in, which lets a project plant a fake binary.
pub fn resolve_binary(env: &Env, name: &str, explicit: Option<&Path>) -> Option<ResolvedBinary> {
    if let Some(path) = explicit.filter(|path| path.is_absolute() && path.is_file()) {
        return Some(ResolvedBinary::new(path.to_path_buf()));
    }

    if let Some(path_var) = env.var("PATH")
        && let Some(search_path) = absolute_path_entries(path_var)
        && let Ok(found) = which::which_in(name, Some(search_path), &env.home)
    {
        return Some(ResolvedBinary::new(found));
    }

    extra_search_dirs(env)
        .iter()
        .find_map(|dir| find_in_dir(env.os, dir, name))
        .map(ResolvedBinary::new)
}

/// `path_var` reduced to its absolute entries, re-joined with the platform separator; `None`
/// when no absolute entry is left.
fn absolute_path_entries(path_var: &str) -> Option<std::ffi::OsString> {
    let absolute = absolute_dirs(path_var);

    if absolute.is_empty() {
        return None;
    }

    std::env::join_paths(absolute).ok()
}

/// The absolute entries of `env`'s `PATH`, in order (see [`resolve_binary`] for why relative ones
/// are left out).
pub fn absolute_path_dirs(env: &Env) -> Vec<PathBuf> {
    env.var("PATH").map(absolute_dirs).unwrap_or_default()
}

fn absolute_dirs(path_var: &str) -> Vec<PathBuf> {
    std::env::split_paths(path_var)
        .filter(|entry| entry.is_absolute())
        .collect()
}

/// Install directories searched after `PATH`, in order.
pub fn extra_search_dirs(env: &Env) -> Vec<PathBuf> {
    let mut dirs = vec![
        env.home_join(".bun/bin"),
        env.home_join(".local/bin"),
        env.home_join(".cargo/bin"),
        env.home_join(".npm-global/bin"),
        // The OpenCode install script's target.
        env.home_join(".opencode/bin"),
    ];

    match env.os {
        OsKind::Windows => {
            if let Some(app_data) = env.var("APPDATA") {
                dirs.push(PathBuf::from(app_data).join("npm"));
            }

            let profile = env.var("USERPROFILE").map_or_else(|| env.home.clone(), PathBuf::from);
            dirs.push(profile.join(".bun").join("bin"));
        }
        OsKind::MacOs => {
            dirs.push(PathBuf::from("/opt/homebrew/bin"));
            dirs.push(PathBuf::from("/usr/local/bin"));
        }
        OsKind::Linux => {
            dirs.push(PathBuf::from("/usr/local/bin"));
        }
    }

    dirs.dedup();
    dirs
}

/// Directories a launcher at `program` may need on the child's `PATH`: its own directory (bun and
/// npm install the interpreter next to their global binaries), then [`extra_search_dirs`].
pub fn launcher_search_dirs(env: &Env, program: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = program.parent().map(Path::to_path_buf).into_iter().collect();

    for dir in extra_search_dirs(env) {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }

    dirs
}

/// Candidate file names for `name` on `os` (Windows tries executables before batch wrappers).
fn candidate_names(os: OsKind, name: &str) -> Vec<String> {
    match os {
        OsKind::Windows => ["exe", "cmd", "bat"]
            .iter()
            .map(|extension| format!("{name}.{extension}"))
            .collect(),
        OsKind::Linux | OsKind::MacOs => vec![name.to_owned()],
    }
}

fn find_in_dir(os: OsKind, dir: &Path, name: &str) -> Option<PathBuf> {
    candidate_names(os, name)
        .into_iter()
        .map(|candidate| dir.join(candidate))
        .find(|path| is_executable_file(path))
}

/// True when `path` is a file (followed through symlinks) that may be run: on Unix, one with an
/// execute bit.
#[cfg(unix)]
pub fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// True when `path` is a file (followed through symlinks) that may be run: on Unix, one with an
/// execute bit.
#[cfg(not(unix))]
pub fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}
