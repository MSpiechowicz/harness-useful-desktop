//! The `harness-memory` server (Harness Memory) on this machine: where it is ([`resolve_memory`]),
//! the version it reports ([`memory_version`]), and both together ([`installed_memory`]), which is
//! what the harnesses' MCP entries name.
//!
//! The app does not ship the server: the setup step and the "Project memory" switch run its
//! installer (see [`super::installer`]). Since the resolved path is written into every harness's
//! MCP file, it is looked up more strictly than a harness binary:
//!
//! - Order: the installer's own location first (`~/.local/bin/harness-memory` from `install.sh`,
//!   `%LOCALAPPDATA%\Programs\harness-memory\harness-memory.exe` from `install.ps1`), then the
//!   absolute `PATH` entries, then the usual install directories ([`extra_search_dirs`]).
//! - Windows: only `harness-memory.exe`; a `.cmd`, `.bat` or `.com` of that name is never used.
//! - Unix: a file that its group or everyone may change, or one in a folder everyone may change
//!   (unless it has the sticky bit), is skipped with a log line, and the next candidate is tried. A
//!   group-writable folder is accepted: with a umask of 002 (user-private groups, the Ubuntu
//!   default) `install.sh` creates `~/.local/bin` as 0775.
//! - Version: [`installed_memory`] takes the first candidate left only when it answers
//!   `harness-memory --version` with a version ([`parse_memory_version`]); otherwise the server
//!   counts as not installed (logged), and no later candidate is tried.

use std::path::{Path, PathBuf};

use super::catalog::MEMORY_BINARY;
use super::detect::version_output;
use crate::env::{Env, OsKind};
use crate::harness::first_semver;
use crate::process::{ProcessRunner, ResolvedBinary, absolute_path_dirs, extra_search_dirs, is_executable_file};

/// The first `harness-memory` executable in the order of the module docs that no other user can
/// change; its version is not checked (see [`installed_memory`]).
pub fn resolve_memory(env: &Env) -> Option<ResolvedBinary> {
    memory_candidates(env)
        .into_iter()
        .filter(|path| is_executable_file(path))
        .find(|path| match others_may_change(path) {
            Some(reason) => {
                tracing::info!(path = %path.display(), reason, "harness-memory candidate skipped");
                false
            }
            None => true,
        })
        .map(|path| ResolvedBinary {
            path,
            is_batch_script: false,
        })
}

/// The server to register and run: [`resolve_memory`]'s binary when it reports a version within
/// the version probe's 5 s limit; `None` (logged) when it does not.
pub async fn installed_memory(env: &Env, runner: &dyn ProcessRunner) -> Option<ResolvedBinary> {
    let binary = resolve_memory(env)?;

    if memory_version(env, runner, &binary).await.is_none() {
        tracing::warn!(
            path = %binary.path.display(),
            "harness-memory did not report a version; treated as not installed"
        );
        return None;
    }

    Some(binary)
}

/// The version `harness-memory --version` of `binary` reports (see [`parse_memory_version`]),
/// within the version probe's 5 s limit.
pub async fn memory_version(env: &Env, runner: &dyn ProcessRunner, binary: &ResolvedBinary) -> Option<String> {
    parse_memory_version(&version_output(env, runner, binary, "--version").await?)
}

/// The version in `harness-memory --version` output: its first non-empty line must be
/// `harness-memory <major.minor.patch[-pre]>` and nothing else. `None` for any other output.
pub fn parse_memory_version(output: &str) -> Option<String> {
    let line = output.lines().map(str::trim).find(|line| !line.is_empty())?;
    let version = line.strip_prefix(MEMORY_BINARY)?.strip_prefix(' ')?.trim();
    let found = first_semver(version)?;

    (found == version).then_some(found)
}

/// Every path the server may be at, in the order of the module docs, without duplicates.
fn memory_candidates(env: &Env) -> Vec<PathBuf> {
    let file_name = match env.os {
        OsKind::Windows => format!("{MEMORY_BINARY}.exe"),
        OsKind::Linux | OsKind::MacOs => MEMORY_BINARY.to_owned(),
    };

    let searched = absolute_path_dirs(env)
        .into_iter()
        .chain(extra_search_dirs(env))
        .map(|dir| dir.join(&file_name));

    let mut candidates = Vec::new();

    for candidate in installer_location(env).into_iter().chain(searched) {
        if candidate.is_absolute() && !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }

    candidates
}

/// Where the server's installer puts it on `env`'s OS; `None` on Windows without `LOCALAPPDATA`.
fn installer_location(env: &Env) -> Option<PathBuf> {
    match env.os {
        OsKind::Linux | OsKind::MacOs => Some(env.home_join(".local/bin").join(MEMORY_BINARY)),
        OsKind::Windows => {
            let local = env.var("LOCALAPPDATA").filter(|value| !value.is_empty())?;

            Some(
                PathBuf::from(local)
                    .join("Programs")
                    .join(MEMORY_BINARY)
                    .join(format!("{MEMORY_BINARY}.exe")),
            )
        }
    }
}

/// Why another user could change what `path` runs, if one could: the file (followed through
/// symlinks) is writable by its group or by everyone, or the folder it is listed in or the folder
/// its target is in is writable by everyone (a folder with the sticky bit is fine).
#[cfg(unix)]
fn others_may_change(path: &Path) -> Option<&'static str> {
    use std::os::unix::fs::PermissionsExt;

    const GROUP_OR_WORLD_WRITE: u32 = 0o022;
    const WORLD_WRITE: u32 = 0o002;
    const STICKY: u32 = 0o1000;

    let mode = |path: &Path| {
        std::fs::metadata(path)
            .map(|metadata| metadata.permissions().mode())
            .ok()
    };

    let Some(file) = mode(path) else {
        return Some("its permissions cannot be read");
    };

    if file & GROUP_OR_WORLD_WRITE != 0 {
        return Some("it is writable by its group or by everyone");
    }

    let target = std::fs::canonicalize(path).ok();
    let folders = [path.parent(), target.as_deref().and_then(Path::parent)];

    for folder in folders.into_iter().flatten() {
        let Some(folder) = mode(folder) else {
            return Some("its folder's permissions cannot be read");
        };

        if folder & WORLD_WRITE != 0 && folder & STICKY == 0 {
            return Some("its folder is writable by everyone");
        }
    }

    None
}

/// Windows file permissions are not checked (only `.exe` files are candidates).
#[cfg(not(unix))]
fn others_may_change(_path: &Path) -> Option<&'static str> {
    None
}
