//! Signed documents: small JSON files in a shared folder that the app window uses only while the
//! SHA-256 of their exact bytes is approved in its config folder, so a file an agent wrote never
//! takes effect unreviewed. Hooks ([`crate::hooks`]) are one kind; every kind follows the folder,
//! approval and team rules of `docs/hooks-format.md` with its own names ([`DocKind`]).
//!
//! - [`DocStore`]: a kind's root (`<root>/<plural>/<id>.json`) and the app config folder, with
//!   the file handling of [`folder`] (reading, the folder checks, a write that still finds the
//!   version it read) and the approval bookkeeping of [`store`].
//! - [`approvals`]: `<kind>-approvals.json` ([`Approvals`]), read by everyone, written by the app
//!   window only. [`snapshots`]: the approved copies a pending edit is compared with.
//! - [`state`]: a kind's own state file in the config folder (versioned, set aside when invalid).
//! - [`fields`] and [`unicode`]: the field rules every kind shares (name, description, scope,
//!   timestamps, invisible characters). [`scope`]: the per-scope limit, the order of documents
//!   and the project root a draft names ([`scope::ScopedDoc`]).
//! - [`team`]: the team copies of documents shared through Harness Memory (`<root>/team`), one
//!   refresh round's lists and how a round is written.
//!
//! The layer never decides what a document means: each kind parses its bytes ([`DocKind::parse`])
//! and builds its own views on top.

pub mod approvals;
mod edit;
pub(crate) mod fields;
mod folder;
pub(crate) mod json_file;
pub(crate) mod listing;
pub mod scope;
pub mod snapshots;
pub mod state;
mod store;
pub mod team;
pub(crate) mod unicode;

use std::fmt::Debug;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub use approvals::Approvals;
pub use folder::owned_by_current_user;
pub(crate) use folder::{changed_since_read, read_file, unsafe_folder, unsafe_root};
pub use scope::ScopedDoc;
pub(crate) use store::{changed_since_shown, not_found};

use crate::app_dirs::shared_data_dir;
use crate::env::Env;
use crate::error::{BusinessError, Result};

/// What differs between kinds of signed documents: names, limits and the parser. Implemented by a
/// unit type per kind (`HookKind`), which the generic types carry.
pub trait DocKind: Copy + Debug + Default + Eq + Send + Sync + 'static {
    /// One document in messages (`hook`).
    const NOUN: &'static str;
    /// Several in messages, the folder inside the root, the `harness-memory` command and the key
    /// of its list and config answers (`hooks`).
    const PLURAL: &'static str;
    /// The variable that overrides the root (`HARNESS_HOOKS_DIR`).
    const ROOT_VAR: &'static str;
    /// The root's folder under the platform data folder (`harness-hooks`).
    const APP_FOLDER: &'static str;
    /// A larger file is invalid and is not parsed.
    const MAX_FILE_BYTES: usize;
    /// Most documents used per scope (global, and each project separately).
    const MAX_PER_SCOPE: usize;
    /// The approvals file in the app config folder (`hook-approvals.json`).
    const APPROVALS_FILE: &'static str;
    /// The folder of approved copies in the app config folder (`hook-approved`).
    const SNAPSHOT_FOLDER: &'static str;
    /// The kind's state file in the app config folder (`hooks-state.json`).
    const STATE_FILE: &'static str;
    /// The key of `<plural> config --json` that says the team's documents are trusted
    /// (`trustTeamHooks`).
    const TRUST_KEY: &'static str;

    /// A parsed document.
    type Doc: ScopedDoc + Clone;

    /// The document in `bytes`, the content of `<id>.json`, or why the file is invalid.
    fn parse(id: &str, bytes: &[u8]) -> std::result::Result<Self::Doc, String>;

    /// The document in a team body (a `git:` repository key as `project`, plus the kind's own team
    /// rules), or why it breaks them.
    fn parse_team(id: &str, bytes: &[u8]) -> std::result::Result<Self::Doc, String>;

    /// The bytes of `doc` as a file: `base` (the object being rewritten, empty for a new file)
    /// with every known field set, unknown fields kept, two-space JSON and a trailing newline.
    fn to_bytes(doc: &Self::Doc, base: Map<String, Value>) -> Vec<u8>;

    /// Why the valid `doc` is never shared with the team (hooks: a Harness family hook), `None`
    /// when it may be.
    fn not_shared(_doc: &Self::Doc) -> Option<String> {
        None
    }

    /// True when the approvals of team copies are kept apart from the local files' (the key
    /// [`team_approval_key`], `team:<id>`, and its own approved copy), so approving a team copy
    /// never approves a local file with the same id and bytes, and a local approval never carries
    /// over to a team copy. False (hooks): one approval per id serves both.
    const SEPARATE_TEAM_APPROVALS: bool = false;

    /// Drops what the kind's state keeps for the team documents `ids` that left this machine (no
    /// local file has their id), next to their approvals. Nothing by default.
    fn forget_team_state(_config_dir: &Path, _ids: &[String]) -> Result<()> {
        Ok(())
    }
}

/// A kind's root ([`DocKind::ROOT_VAR`] when it is absolute after `~` expansion, else the
/// platform data folder followed by [`DocKind::APP_FOLDER`], see [`shared_data_dir`]). `None` on
/// Windows without `LOCALAPPDATA`: the kind is then unavailable.
pub fn doc_root<K: DocKind>(env: &Env) -> Option<PathBuf> {
    shared_data_dir(env, K::ROOT_VAR, K::APP_FOLDER)
}

/// A kind's folder (`<root>/<plural>`) and the app config folder that holds the approvals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocStore<K: DocKind> {
    root: PathBuf,
    config_dir: PathBuf,
    kind: PhantomData<K>,
}

impl<K: DocKind> DocStore<K> {
    /// A store for the root `root` and the app config folder `config_dir`.
    pub fn new(root: impl Into<PathBuf>, config_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            config_dir: config_dir.into(),
            kind: PhantomData,
        }
    }

    /// The store at [`doc_root`]; `None` when there is no such folder (Windows without
    /// `LOCALAPPDATA`).
    pub fn from_env(env: &Env, config_dir: impl Into<PathBuf>) -> Option<Self> {
        doc_root::<K>(env).map(|root| Self::new(root, config_dir))
    }

    /// The root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<root>/<plural>`, where the files are.
    pub fn docs_dir(&self) -> PathBuf {
        self.root.join(K::PLURAL)
    }

    /// The app config folder (approvals, approved copies and the state file).
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }
}

/// One `<id>.json` file as read.
#[derive(Debug, Clone)]
pub(crate) struct DocFile {
    pub id: String,
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub unreadable: bool,
}

impl DocFile {
    /// The JSON object in the file (empty when it holds none), so a rewrite keeps its unknown
    /// fields.
    pub fn object(&self) -> Map<String, Value> {
        json_object(&self.bytes)
    }
}

/// The JSON object in `bytes`, empty when they hold none.
pub(crate) fn json_object(bytes: &[u8]) -> Map<String, Value> {
    match serde_json::from_slice::<Value>(bytes) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

/// `value` as the apps write their files: two-space JSON and a trailing newline. `what` names it
/// in the error.
pub(crate) fn pretty_json<T: Serialize + ?Sized>(value: &T, what: &str) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| BusinessError::Internal(format!("encoding {what}: {error}")))?;
    bytes.push(b'\n');

    Ok(bytes)
}

/// True for a document id: 32 lowercase hex characters.
pub fn is_doc_id(text: &str) -> bool {
    text.len() == 32 && is_lower_hex(text)
}

/// The prefix of a team copy's key in the approvals file of a kind that keeps team approvals
/// apart ([`DocKind::SEPARATE_TEAM_APPROVALS`]).
pub const TEAM_APPROVAL_PREFIX: &str = "team:";

/// The key the approval of the team copy of document `id` is kept under: `team:<id>` for a kind
/// that keeps team approvals apart, else `id` (the local file's key).
pub fn team_approval_key<K: DocKind>(id: &str) -> String {
    if K::SEPARATE_TEAM_APPROVALS {
        format!("{TEAM_APPROVAL_PREFIX}{id}")
    } else {
        id.to_owned()
    }
}

/// True for a key of the approvals file: a document id, or `team:` followed by one.
pub(crate) fn is_approval_key(key: &str) -> bool {
    is_doc_id(key.strip_prefix(TEAM_APPROVAL_PREFIX).unwrap_or(key))
}

/// True for a lowercase SHA-256: 64 lowercase hex characters.
pub fn is_sha256(text: &str) -> bool {
    text.len() == 64 && is_lower_hex(text)
}

fn is_lower_hex(text: &str) -> bool {
    text.bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Lowercase hex SHA-256 of `bytes` (a document's approval hash).
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The project root of `folder` (see "Project identity" in the format doc): its real path
/// (symbolic links resolved, no `\\?\` prefix on Windows), then the nearest ancestor-or-self that
/// contains `.git` (a folder or a file), else the folder itself. `NotFound` for a missing path,
/// `InvalidInput` for one that is not a folder.
pub fn project_identity(folder: &Path) -> Result<PathBuf> {
    let real = dunce::canonicalize(folder)
        .map_err(|error| BusinessError::from_io(format!("resolving {}", folder.display()), error))?;

    if !real.is_dir() {
        return Err(BusinessError::InvalidInput(format!(
            "{} is not a folder",
            folder.display()
        )));
    }

    let root = real
        .ancestors()
        .find(|dir| std::fs::metadata(dir.join(".git")).is_ok_and(|metadata| metadata.is_dir() || metadata.is_file()))
        .unwrap_or(&real)
        .to_path_buf();

    Ok(root)
}

/// `InvalidInput` unless `id` is a document id.
pub(crate) fn check_id<K: DocKind>(id: &str) -> Result<()> {
    if !is_doc_id(id) {
        return Err(BusinessError::InvalidInput(format!(
            "a {} id is 32 lowercase hex characters",
            K::NOUN
        )));
    }

    Ok(())
}
