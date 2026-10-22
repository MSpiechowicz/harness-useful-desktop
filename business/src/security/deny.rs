//! The never-read list and [`SafeReader`].
//!
//! **Every file read in the business layer goes through [`SafeReader`].** It refuses deny-listed
//! files (databases, logs, keys, credential stores, blob caches), checks the canonical path and
//! every symlink hop, refuses non-regular files and caps the size, so a credential store can never
//! be opened by mistake — not even through a symlink.

mod reader;

use std::fmt;
use std::path::{Component, Path};
use std::sync::RwLock;

pub use self::reader::SafeReader;
pub(crate) use self::reader::add_claude_global_file;

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
    CLAUDE_GLOBAL_FILE,
    "mcp-needs-auth-cache.json",
    "policy-limits.json",
    "policy-limits.stamp.json",
];

/// File-name prefixes (lowercase) that are never read.
const DENIED_PREFIXES: &[&str] = &[
    ".env.",
    "id_rsa",
    "id_ed25519",
    // Claude Code's copies of its global file: backups (`.claude.json.backup.<timestamp>`),
    // corrupted copies (`.claude.json.corrupted.<timestamp>`) and its temporary files.
    ".claude.json.",
];

/// Claude Code's global file (`~/.claude.json`): the account, device ids, caches and every
/// project's state. Denied everywhere; only [`SafeReader::read_claude_global_file`] opens it.
pub(crate) const CLAUDE_GLOBAL_FILE: &str = ".claude.json";

/// File names (lowercase) with this prefix and suffix are never read: the OpenCode desktop app's
/// state (`opencode.settings.dat`, `opencode.global.dat`, `opencode.workspace.<id>.dat`).
const DENIED_PREFIX_SUFFIXES: &[(&str, &str)] = &[("opencode.", ".dat")];

/// Directory names (lowercase) whose contents are never read: OMP's terminal and search stores.
const DENIED_DIRS: &[&str] = &["terminal-sessions", "url-search"];

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
/// browser extension's state, the IDE connection locks, which hold an auth token, pasted text,
/// shell environment snapshots, file edit history and per-session environments
/// (`~/.claude/backups`, `statsig`, `debug`, `uploads`, `image-cache`, `chrome`, `ide`,
/// `paste-cache`, `shell-snapshots`, `file-history`, `session-env`). The names alone are too
/// generic.
const DENIED_CLAUDE_DIRS: &[&str] = &[
    "backups",
    "statsig",
    "debug",
    "uploads",
    "image-cache",
    "chrome",
    "ide",
    "paste-cache",
    "shell-snapshots",
    "file-history",
    "session-env",
];

/// Prefix of directory names (lowercase) never read when their direct parent is `.claude`: the
/// feedback bundles (`~/.claude/feedback-*`).
const DENIED_CLAUDE_DIR_PREFIX: &str = "feedback";

/// File names (lowercase) never read when their direct parent is `.claude`: the prompts typed into
/// Claude Code, the remotely managed settings and the usage statistics cache.
const DENIED_CLAUDE_FILES: &[&str] = &["history.jsonl", "remote-settings.json", "stats-cache.json"];

/// Maximum symlink hops followed when checking link targets.
pub(crate) const MAX_LINK_HOPS: usize = 40;

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

/// Checks `path` (as given, without touching the filesystem) only against the private names of a
/// Claude Code config directory: a folder named `.claude` or a root registered with
/// [`add_claude_root`]. Unlike [`is_denied`] it ignores the rules that match a name anywhere among
/// the ancestors (`projects`, `agent`, `opencode`, `terminal-sessions`), so a project's absolute
/// path that happens to cross such a folder is not caught. Names are compared case-insensitively.
pub(crate) fn is_denied_under_claude_root(path: &Path) -> Option<DenyReason> {
    let components = lowercase_components(path);

    let (file_name, parents) = components.split_last()?;

    if DENIED_CLAUDE_FILES.contains(&file_name.as_str()) && is_claude_root(parents) {
        return Some(DenyReason::FileName(file_name.clone()));
    }

    for (index, dir) in parents.iter().enumerate() {
        let ancestors = &parents[..index];

        if is_claude_config_private_dir(dir, ancestors) {
            return Some(DenyReason::Directory(dir.clone()));
        }

        // A session's full tool outputs, `<config>/projects/<slug>/<session>/tool-results`.
        let below_claude_projects =
            (1..ancestors.len()).any(|split| ancestors[split] == "projects" && is_claude_root(&ancestors[..split]));
        if dir == "tool-results" && below_claude_projects {
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
    if is_claude_config_private_dir(dir, ancestors) {
        return true;
    }

    dir == "tool-results" && ancestors.iter().any(|ancestor| ancestor == "projects")
}

/// True for a [`DENIED_CLAUDE_DIRS`] or `feedback*` folder directly in a Claude config directory
/// (see [`is_claude_root`]) and for its `plugins/data`.
fn is_claude_config_private_dir(dir: &str, ancestors: &[String]) -> bool {
    let private_name = DENIED_CLAUDE_DIRS.contains(&dir) || dir.starts_with(DENIED_CLAUDE_DIR_PREFIX);
    if private_name && is_claude_root(ancestors) {
        return true;
    }

    matches!(ancestors, [config @ .., plugins] if dir == "data" && plugins == "plugins" && is_claude_root(config))
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
