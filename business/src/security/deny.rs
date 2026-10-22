//! The never-read list and [`SafeReader`].
//!
//! **Every file read in the business layer goes through [`SafeReader`].** It refuses deny-listed
//! files (databases, logs, keys, credential stores, blob caches), checks the canonical path and
//! every symlink hop, refuses non-regular files and caps the size, so a credential store can never
//! be opened by mistake — not even through a symlink.

use std::fmt;
use std::fs::{File, Metadata, OpenOptions};
use std::io::{BufRead, BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::RwLock;

use crate::error::{BusinessError, Result};

/// Extensions (lowercase, without dot) that are never read.
const DENIED_EXTENSIONS: &[&str] = &[
    "db",
    "db-wal",
    "db-shm",
    "db-journal",
    "sqlite",
    "sqlite3",
    "log",
    "pem",
    "key",
    "p12",
    "pfx",
    // Claude Code's memory dumps (`/heapdump`), which can hold tokens.
    "heapsnapshot",
];

/// Exact file names (lowercase) that are never read.
const DENIED_NAMES: &[&str] = &[
    "auth.json",
    "mcp-auth.json",
    "install-id",
    "auth-token",
    "models-store.json",
    "gpu_cache.json",
    "credentials",
    "credentials.json",
    ".env",
    // Claude Code's login store and its cached account and policy state.
    ".credentials.json",
    "mcp-needs-auth-cache.json",
    "policy-limits.json",
    "policy-limits.stamp.json",
];

/// File-name prefixes (lowercase) that are never read.
const DENIED_PREFIXES: &[&str] = &[
    ".env.",
    "id_rsa",
    "id_ed25519",
    // Claude Code's copies of its global file (`.claude.json.backup.<timestamp>`).
    ".claude.json.backup",
];

/// File names (lowercase) with this prefix and suffix are never read: the OpenCode desktop app's
/// state (`opencode.settings.dat`, `opencode.global.dat`, `opencode.workspace.<id>.dat`).
const DENIED_PREFIX_SUFFIXES: &[(&str, &str)] = &[("opencode.", ".dat")];

/// Directory names (lowercase) whose contents are never read: OMP's terminal and search stores,
/// and Claude Code's pasted text, shell environment snapshots, file edit history and per-session
/// environments.
const DENIED_DIRS: &[&str] = &[
    "terminal-sessions",
    "url-search",
    "paste-cache",
    "shell-snapshots",
    "file-history",
    "session-env",
];

/// Directory names (lowercase) whose contents are never read when their direct parent is
/// `opencode`: transcripts, cached repositories, project snapshots, full tool outputs and session
/// plans (`<data>/opencode/storage`, `repos`, `snapshot`, `tool-output`, `plans`). The names alone
/// are too generic.
const DENIED_OPENCODE_DIRS: &[&str] = &["storage", "repos", "snapshot", "tool-output", "plans"];

/// File names (lowercase) never read when their direct parent is `opencode`: the prompts typed
/// into OpenCode (`<state>/opencode/prompt-history.jsonl`).
const DENIED_OPENCODE_FILES: &[&str] = &["prompt-history.jsonl"];

/// The default name of Claude Code's config directory (`~/.claude`), whose private children the lists
/// below name. A config directory `CLAUDE_CONFIG_DIR` moves elsewhere is registered with
/// [`add_claude_root`] and covered the same way.
const CLAUDE_DIR: &str = ".claude";

/// Claude Code config directories with another name (`CLAUDE_CONFIG_DIR`), as lowercase path
/// components. Roots are only ever added, so registering one never weakens the list.
static CLAUDE_ROOTS: RwLock<Vec<Vec<String>>> = RwLock::new(Vec::new());

/// Directory names (lowercase) whose contents are never read when their direct parent is
/// `.claude`: backups of the global file, telemetry, debug logs, uploaded and cached images, the
/// browser extension's state and the IDE connection locks, which hold an auth token
/// (`~/.claude/backups`, `statsig`, `debug`, `uploads`, `image-cache`, `chrome`, `ide`). The names
/// alone are too generic.
const DENIED_CLAUDE_DIRS: &[&str] = &["backups", "statsig", "debug", "uploads", "image-cache", "chrome", "ide"];

/// Prefix of directory names (lowercase) never read when their direct parent is `.claude`: the
/// feedback bundles (`~/.claude/feedback-*`).
const DENIED_CLAUDE_DIR_PREFIX: &str = "feedback";

/// File names (lowercase) never read when their direct parent is `.claude`: the prompts typed into
/// Claude Code, the remotely managed settings and the usage statistics cache.
const DENIED_CLAUDE_FILES: &[&str] = &["history.jsonl", "remote-settings.json", "stats-cache.json"];

/// Maximum symlink hops followed when checking link targets.
const MAX_LINK_HOPS: usize = 40;

/// Why a path is on the deny-list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenyReason {
    /// The extension marks a database, log or key file.
    Extension(String),
    /// The file name is a known credential or private store.
    FileName(String),
    /// The path lies inside a private directory.
    Directory(String),
}

impl fmt::Display for DenyReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Extension(extension) => write!(f, "*.{extension} files are never read"),
            Self::FileName(name) => write!(f, "{name} is never read"),
            Self::Directory(name) => write!(f, "files under {name}/ are never read"),
        }
    }
}

/// Registers `root` as a Claude Code config directory (`CLAUDE_CONFIG_DIR`): the private names
/// [`is_denied`] refuses directly in a folder named `.claude` are refused below `root` too, whatever
/// its name. [`crate::env::Env`] registers its root (and the root's canonical path) when it is
/// captured or built, so every later check covers it. Registering the same root again changes
/// nothing.
pub fn add_claude_root(root: &Path) {
    let mut roots = vec![lowercase_components(root)];
    if let Ok(canonical) = dunce::canonicalize(root) {
        roots.push(lowercase_components(&canonical));
    }

    let mut registered = CLAUDE_ROOTS.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    for root in roots {
        if !root.is_empty() && !registered.contains(&root) {
            registered.push(root);
        }
    }
}

/// The normal components of `path`, lowercased (drive prefixes and roots are dropped).
fn lowercase_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect()
}

/// True when `ancestors` is a Claude Code config directory: a folder named `.claude` or a root
/// registered with [`add_claude_root`].
fn is_claude_root(ancestors: &[String]) -> bool {
    if ancestors.last().is_some_and(|parent| parent == CLAUDE_DIR) {
        return true;
    }

    let roots = CLAUDE_ROOTS.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    roots.iter().any(|root| root.as_slice() == ancestors)
}

/// Checks `path` (as given, without touching the filesystem) against the deny-list.
/// Names are compared case-insensitively.
pub fn is_denied(path: &Path) -> Option<DenyReason> {
    let components = lowercase_components(path);

    let (file_name, parents) = components.split_last()?;

    if let Some(reason) = denied_file_name(file_name) {
        return Some(reason);
    }

    if DENIED_OPENCODE_FILES.contains(&file_name.as_str()) && parents.last().is_some_and(|parent| parent == "opencode")
    {
        return Some(DenyReason::FileName(file_name.clone()));
    }

    if DENIED_CLAUDE_FILES.contains(&file_name.as_str()) && is_claude_root(parents) {
        return Some(DenyReason::FileName(file_name.clone()));
    }

    for (index, dir) in parents.iter().enumerate() {
        if DENIED_DIRS.contains(&dir.as_str()) {
            return Some(DenyReason::Directory(dir.clone()));
        }

        // Agent blob stores (`~/.omp/agent/blobs/`).
        if dir == "blobs" && parents[..index].iter().any(|ancestor| ancestor == "agent") {
            return Some(DenyReason::Directory(dir.clone()));
        }

        // OpenCode transcripts, cached repositories, snapshots, tool outputs and plans
        // (`<data>/opencode/storage/`).
        let parent = index.checked_sub(1).map(|parent| parents[parent].as_str());
        if DENIED_OPENCODE_DIRS.contains(&dir.as_str()) && parent == Some("opencode") {
            return Some(DenyReason::Directory(dir.clone()));
        }

        // OpenCode's plugin packages (`~/.cache/opencode/node_modules/`).
        if dir == "node_modules" && is_opencode_cache(&parents[..index]) {
            return Some(DenyReason::Directory(dir.clone()));
        }

        if is_claude_private_dir(dir, &parents[..index]) {
            return Some(DenyReason::Directory(dir.clone()));
        }
    }

    None
}

/// True for Claude Code's private folders: a [`DENIED_CLAUDE_DIRS`] or `feedback*` folder directly in
/// a Claude config directory (see [`is_claude_root`]), its `plugins/data` (the plugins' own data)
/// and a session's `tool-results` below a `projects` folder (full tool outputs,
/// `~/.claude/projects/<slug>/<session>/tool-results`).
fn is_claude_private_dir(dir: &str, ancestors: &[String]) -> bool {
    let private_name = DENIED_CLAUDE_DIRS.contains(&dir) || dir.starts_with(DENIED_CLAUDE_DIR_PREFIX);
    if private_name && is_claude_root(ancestors) {
        return true;
    }

    if let [config @ .., plugins] = ancestors
        && dir == "data"
        && plugins == "plugins"
        && is_claude_root(config)
    {
        return true;
    }

    dir == "tool-results" && ancestors.iter().any(|ancestor| ancestor == "projects")
}

/// True when `ancestors` ends in `.cache/opencode` (or `cache/opencode`).
fn is_opencode_cache(ancestors: &[String]) -> bool {
    match ancestors {
        [.., cache, opencode] => opencode == "opencode" && (cache == ".cache" || cache == "cache"),
        _ => false,
    }
}

fn denied_file_name(name: &str) -> Option<DenyReason> {
    let prefixed = DENIED_PREFIXES.iter().any(|prefix| name.starts_with(prefix));
    let prefixed_and_suffixed = DENIED_PREFIX_SUFFIXES
        .iter()
        .any(|(prefix, suffix)| name.starts_with(prefix) && name.ends_with(suffix));

    if DENIED_NAMES.contains(&name) || prefixed || prefixed_and_suffixed {
        return Some(DenyReason::FileName(name.to_owned()));
    }

    let extension = Path::new(name).extension()?.to_str()?;
    DENIED_EXTENSIONS
        .contains(&extension)
        .then(|| DenyReason::Extension(extension.to_owned()))
}

/// Guarded file reads. See the module docs.
#[derive(Debug, Clone, Copy, Default)]
pub struct SafeReader;

impl SafeReader {
    /// Default size cap for [`SafeReader::read_to_string`] (2 MiB).
    pub const DEFAULT_MAX_BYTES: u64 = 2 * 1024 * 1024;

    /// Reads a whole UTF-8 file of at most `max_bytes` bytes.
    ///
    /// Errors: `Forbidden` for deny-listed paths (including via symlinks), `InvalidInput` for
    /// non-regular files, oversized files and invalid UTF-8, `NotFound` for missing files.
    pub fn read_to_string(path: &Path, max_bytes: u64) -> Result<String> {
        let (file, metadata) = open_checked(path)?;

        if metadata.len() > max_bytes {
            return Err(too_large(path, max_bytes));
        }

        let mut bytes = Vec::new();
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| BusinessError::from_io(format!("reading {}", path.display()), error))?;

        // The file may have grown between the size check and the read.
        if bytes.len() as u64 > max_bytes {
            return Err(too_large(path, max_bytes));
        }

        String::from_utf8(bytes)
            .map_err(|_| BusinessError::InvalidInput(format!("{} is not valid UTF-8", path.display())))
    }

    /// Opens a file for a bounded streaming read (session transcripts, untracked files). The
    /// caller caps how much it reads. Same access checks as [`SafeReader::read_to_string`].
    pub fn open(path: &Path) -> Result<File> {
        open_checked(path).map(|(file, _)| file)
    }

    /// Reads up to `n` lines from the start of a file, splitting on `\n` only (Unicode line
    /// separators stay inside lines) and stripping one trailing `\r` per line.
    ///
    /// A line longer than `max_line_bytes` fails with `InvalidInput`; fewer than `n` lines are
    /// returned at end of file. Same access checks as [`SafeReader::read_to_string`].
    pub fn read_first_lines(path: &Path, n: usize, max_line_bytes: usize) -> Result<Vec<String>> {
        let (file, _) = open_checked(path)?;
        let mut reader = BufReader::new(file);
        let mut lines = Vec::with_capacity(n);
        let mut buffer = Vec::new();

        while lines.len() < n {
            buffer.clear();

            // Room for the content plus a `\r\n` terminator; anything longer is over the limit.
            let limit = max_line_bytes as u64 + 2;
            let read = (&mut reader)
                .take(limit)
                .read_until(b'\n', &mut buffer)
                .map_err(|error| BusinessError::from_io(format!("reading {}", path.display()), error))?;

            if read == 0 {
                break;
            }

            if buffer.last() == Some(&b'\n') {
                buffer.pop();
            }

            if buffer.last() == Some(&b'\r') {
                buffer.pop();
            }

            if buffer.len() > max_line_bytes {
                return Err(BusinessError::InvalidInput(format!(
                    "{} has a line longer than {max_line_bytes} bytes",
                    path.display()
                )));
            }

            let line = String::from_utf8(std::mem::take(&mut buffer))
                .map_err(|_| BusinessError::InvalidInput(format!("{} is not valid UTF-8", path.display())))?;
            lines.push(line);
        }

        Ok(lines)
    }
}

fn too_large(path: &Path, max_bytes: u64) -> BusinessError {
    BusinessError::InvalidInput(format!("{} is larger than {max_bytes} bytes", path.display()))
}

fn forbidden(path: &Path, reason: &DenyReason) -> BusinessError {
    BusinessError::Forbidden(format!("{}: {reason}", path.display()))
}

/// Applies every check and opens the canonical path for reading.
fn open_checked(path: &Path) -> Result<(File, Metadata)> {
    if let Some(reason) = is_denied(path) {
        return Err(forbidden(path, &reason));
    }

    check_link_chain(path)?;

    let canonical = dunce::canonicalize(path)
        .map_err(|error| BusinessError::from_io(format!("resolving {}", path.display()), error))?;

    if let Some(reason) = is_denied(&canonical) {
        return Err(forbidden(path, &reason));
    }

    // Checked before opening so a FIFO or device is never opened (opening a FIFO can block).
    let metadata = std::fs::metadata(&canonical).map_err(|error| inspect_error(path, error))?;
    ensure_regular(path, &metadata)?;

    let file =
        open_read(&canonical).map_err(|error| BusinessError::from_io(format!("opening {}", path.display()), error))?;

    // Re-checked on the open handle: the path may have been swapped after the first check.
    let metadata = file.metadata().map_err(|error| inspect_error(path, error))?;
    ensure_regular(path, &metadata)?;

    Ok((file, metadata))
}

fn inspect_error(path: &Path, error: std::io::Error) -> BusinessError {
    BusinessError::from_io(format!("inspecting {}", path.display()), error)
}

fn ensure_regular(path: &Path, metadata: &Metadata) -> Result<()> {
    if metadata.is_file() {
        return Ok(());
    }

    Err(BusinessError::InvalidInput(format!(
        "{} is not a regular file",
        path.display()
    )))
}

/// Follows a symlink chain hop by hop and refuses it when any hop names a denied file.
fn check_link_chain(path: &Path) -> Result<()> {
    let mut current = path.to_path_buf();

    for _ in 0..MAX_LINK_HOPS {
        let is_link = std::fs::symlink_metadata(&current).is_ok_and(|metadata| metadata.file_type().is_symlink());
        if !is_link {
            return Ok(());
        }

        let target = std::fs::read_link(&current)
            .map_err(|error| BusinessError::from_io(format!("resolving {}", path.display()), error))?;
        current = resolve_link_target(&current, target);

        if let Some(reason) = is_denied(&current) {
            return Err(forbidden(path, &reason));
        }
    }

    Err(BusinessError::InvalidInput(format!(
        "{} has too many symlink hops",
        path.display()
    )))
}

fn resolve_link_target(link: &Path, target: PathBuf) -> PathBuf {
    if target.is_absolute() {
        return target;
    }

    match link.parent() {
        Some(parent) => parent.join(target),
        None => target,
    }
}

#[cfg(unix)]
fn open_read(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    // Non-blocking open so a FIFO swapped in after the type check cannot hang the reader.
    OpenOptions::new()
        .read(true)
        .custom_flags(nix::fcntl::OFlag::O_NONBLOCK.bits())
        .open(path)
}

#[cfg(not(unix))]
fn open_read(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).open(path)
}
