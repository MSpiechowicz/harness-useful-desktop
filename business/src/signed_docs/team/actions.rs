//! The local halves of what the user does with team documents, the same for every kind (the
//! server half runs `harness-memory <plural> put|delete`, see
//! [`crate::ecosystem::memory_team_docs`]): sharing a local document ([`share_request`],
//! [`record_share`]), forgetting one that left the team ([`forget_team_doc`]) and copying a team
//! document to the local folder ([`copy_team_doc`]). App window only, under its write lock for the
//! kind.
//!
//! A shared body is the local file's exact bytes with only a project document's `project`
//! replaced by the repository key ([`share_body`]); a copy is the team body with the local
//! project root as `project` ([`local_copy`]). Every other field, unknown ones included, travels
//! as written.

use std::marker::PhantomData;

use serde_json::{Map, Value};

use super::folder::TeamFolder;
use super::{ProjectKey, TeamRow, with_project};
use crate::error::{BusinessError, Result};
use crate::model::HookScope;
use crate::signed_docs::edit::file_at;
use crate::signed_docs::state::{TeamState, load_kind_state, save_state};
use crate::signed_docs::{DocKind, DocStore, ScopedDoc, changed_since_shown, json_object, not_found, read_file};

/// A local document the user shares (see [`share_request`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareRequest<K: DocKind> {
    /// The document id (the team copy keeps it).
    pub id: String,
    /// The SHA-256 of the local file shared.
    pub local_sha256: String,
    /// The project root of a project document (`--project`), `None` for a global one.
    pub project: Option<String>,
    /// The SHA-256 of the team copy this machine has, when the team already has the document: the
    /// share then updates it (`--expect-sha`).
    pub team_sha256: Option<String>,
    bytes: Vec<u8>,
    kind: PhantomData<K>,
}

impl<K: DocKind> ShareRequest<K> {
    /// The body to send: the file, a project document's `project` replaced by `key` (see
    /// [`share_body`]).
    pub fn body(&self, key: Option<&ProjectKey>) -> Result<Vec<u8>> {
        share_body::<K>(&self.id, &self.bytes, key)
    }
}

/// The body the local file `bytes` (document `id`) is shared as: unchanged for a global document,
/// its `project` replaced by `key` for a project one. `Forbidden` for a document its kind never
/// shares ([`DocKind::not_shared`]) and for a project without an `origin` remote (a `path:` key),
/// `InvalidInput` for a file that is not a valid document or a project document without `key`.
pub fn share_body<K: DocKind>(id: &str, bytes: &[u8], key: Option<&ProjectKey>) -> Result<Vec<u8>> {
    let doc = K::parse(id, bytes).map_err(BusinessError::InvalidInput)?;

    if let Some(reason) = K::not_shared(&doc) {
        return Err(BusinessError::Forbidden(reason));
    }

    let body = match doc.doc_scope() {
        HookScope::Global => bytes.to_vec(),
        HookScope::Project => {
            let key =
                key.ok_or_else(|| BusinessError::InvalidInput("the project's repository is unknown".to_owned()))?;

            if !key.can_share() {
                return Err(BusinessError::Forbidden(format!(
                    "the repository has no origin remote, so its {} stay on this computer",
                    K::PLURAL
                )));
            }

            with_project::<K>(bytes, &key.key)?
        }
    };

    K::parse_team(id, &body).map_err(BusinessError::InvalidInput)?;

    Ok(body)
}

/// The local document `id`, at version `sha256`, ready to share: valid, used (not over its scope's
/// limit), approved and one its kind shares. `Conflict` when it changed since `sha256`, `NotFound`
/// when there is no such file, `InvalidInput` for an invalid or pending one, `Forbidden` for one
/// its kind never shares or a folder that is not read.
pub fn share_request<K: DocKind>(
    store: &DocStore<K>,
    folder: &TeamFolder<K>,
    id: &str,
    sha256: &str,
) -> Result<ShareRequest<K>> {
    crate::signed_docs::check_id::<K>(id)?;

    let (sorted, approvals) = store.sorted_files().map_err(BusinessError::Forbidden)?;
    let breaks = || BusinessError::InvalidInput(format!("the {} breaks the format", K::NOUN));

    let used = sorted.used.iter().find(|(_, file)| file.id == id);
    let invalid = sorted.invalid.iter().find(|(file, _, _)| file.id == id);

    let (doc, file) = match (used, invalid) {
        (Some((doc, file)), _) => (Some(doc), file),
        (None, Some((file, _, _))) => (None, file),
        (None, None) => return Err(not_found::<K>()),
    };

    if file.sha256 != sha256 {
        return Err(changed_since_shown::<K>());
    }

    let doc = doc.ok_or_else(breaks)?;

    if let Some(reason) = K::not_shared(doc) {
        return Err(BusinessError::Forbidden(reason));
    }

    if !approvals.is_approved(id, sha256) {
        return Err(BusinessError::InvalidInput(format!(
            "the {} is waiting for review: approve it first",
            K::NOUN
        )));
    }

    let file = file_at(store, id, sha256)?;

    let project = match doc.doc_scope() {
        HookScope::Global => None,
        HookScope::Project => doc.doc_project().map(str::to_owned),
    };

    Ok(ShareRequest {
        id: id.to_owned(),
        local_sha256: sha256.to_owned(),
        project,
        team_sha256: folder.row(id).ok().map(|row| row.sha256),
        bytes: file.bytes,
        kind: PhantomData,
    })
}

/// After a share: keeps the `row` the server stored as the team copy, the repository `key` of the
/// project root `project`, and the local version shared (`sharedFrom` in the kind's state `S`).
pub fn record_share<K: DocKind, S: TeamState>(
    store: &DocStore<K>,
    folder: &TeamFolder<K>,
    row: &TeamRow,
    local_sha256: &str,
    project: Option<(&str, &ProjectKey)>,
) -> Result<()> {
    folder.write_row(row)?;

    if let Some((root, key)) = project {
        let mut projects = folder.projects();
        projects.insert(root.to_owned(), key.clone());
        folder.save_projects(&projects)?;
    }

    let mut state = load_kind_state::<K, S>(store.config_dir())?;
    state.set_shared_from(&row.id, local_sha256);
    save_state::<K, S>(store.config_dir(), &state)
}

/// After a removal from the team: drops the copy of team document `id`, its entries in the
/// kind's state `S`, and its approval (see [`DocStore::forget_team_approvals`]).
pub fn forget_team_doc<K: DocKind, S: TeamState>(store: &DocStore<K>, folder: &TeamFolder<K>, id: &str) -> Result<()> {
    folder.remove(id)?;

    let mut state = load_kind_state::<K, S>(store.config_dir())?;
    state.forget_team(id);
    save_state::<K, S>(store.config_dir(), &state)?;

    store.forget_team_approvals(&[id.to_owned()])
}

/// The local file the team body `bytes` of document `id` is copied to, as a document and the
/// object it was read from (for its unknown fields): a project document's `project` becomes
/// `root`. `InvalidInput` for a body that breaks the team rules, or a project document without
/// `root`.
pub(crate) fn local_copy<K: DocKind>(
    id: &str,
    bytes: &[u8],
    root: Option<&str>,
) -> Result<(K::Doc, Map<String, Value>)> {
    let team = K::parse_team(id, bytes).map_err(BusinessError::InvalidInput)?;

    let local = match team.doc_scope() {
        HookScope::Global => bytes.to_vec(),
        HookScope::Project => {
            let root = root.ok_or_else(|| {
                BusinessError::InvalidInput(format!("open the project to copy one of its remote {}", K::PLURAL))
            })?;

            with_project::<K>(bytes, root)?
        }
    };

    let doc = K::parse(id, &local).map_err(BusinessError::InvalidInput)?;

    Ok((doc, json_object(&local)))
}

/// Copies the team document `id` (version `sha256`) to the local folder as an approved document
/// with the same id, which then wins over the team copy. `may_copy` says whether the team copy
/// may be used here (trusted or approved, never a pending one), so a copy never approves what the
/// user has not. A project document needs `project`, a project root and its repository key, which
/// must be the document's. Recorded as shared from the new file (`sharedFrom` in `S`), so a later
/// local edit offers "Update team copy". `Conflict` when the copy changed, `NotFound` without one,
/// `InvalidInput` when `may_copy` refuses, without a matching project or when a local file has the
/// id. Nothing is written when it fails.
pub fn copy_team_doc<K: DocKind, S: TeamState>(
    store: &DocStore<K>,
    folder: &TeamFolder<K>,
    id: &str,
    sha256: &str,
    project: Option<(&str, &str)>,
    may_copy: impl FnOnce() -> Result<bool>,
) -> Result<()> {
    let row = folder.row_at(id, sha256)?;

    if read_file::<K>(&store.file_path(id), id).is_some() {
        return Err(BusinessError::InvalidInput(format!(
            "a local {} already has this id",
            K::NOUN
        )));
    }

    if !may_copy()? {
        return Err(BusinessError::InvalidInput(format!(
            "approve the remote {} before you copy it",
            K::NOUN
        )));
    }

    let team = K::parse_team(id, row.body.as_bytes()).map_err(BusinessError::InvalidInput)?;

    let root = match (team.doc_scope(), project) {
        (HookScope::Global, _) => None,
        (HookScope::Project, Some((root, key))) if team.doc_project() == Some(key) => Some(root),
        (HookScope::Project, Some(_)) => {
            return Err(BusinessError::InvalidInput(format!(
                "the {} belongs to another repository",
                K::NOUN
            )));
        }
        (HookScope::Project, None) => None,
    };

    let (doc, base) = local_copy::<K>(id, row.body.as_bytes(), root)?;
    let local_sha256 = store.create_approved_from(&doc, base)?;

    let mut state = load_kind_state::<K, S>(store.config_dir())?;
    state.set_shared_from(id, &local_sha256);
    save_state::<K, S>(store.config_dir(), &state)
}
