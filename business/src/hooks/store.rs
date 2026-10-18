//! The hooks folder: listing the files with their status, and writes and deletes by the app
//! window ([`HookAuthor::Desktop`]) or by the MCP server on behalf of an agent
//! ([`HookAuthor::Agent`]).
//!
//! - Only names that are exactly `<id>.json` are read, so temp files, leftovers and editor files
//!   are ignored, and so is anything that is not a regular file (symbolic links included). On Unix
//!   the whole folder is ignored while group or others can write to it.
//! - Per scope (global, each project) at most [`MAX_HOOKS_PER_SCOPE`] hooks are used: approved
//!   ones first, each group by `createdAt` then `id`; the rest are listed as invalid. The app
//!   window refuses a new hook when a scope holds the limit of approved hooks, an agent when it
//!   holds the limit of valid ones.
//! - Every write sets `updatedBy` to its author (a new file gets it as `createdBy` too) and keeps
//!   `createdAt`, `createdBy`, `seed`, `requires` and unknown top-level fields of the file it
//!   rewrites. An edit or an agent's delete names the SHA-256 of the version it changes.
//! - Only the app window approves: its writes store the approval and a copy of the approved bytes
//!   with the file, so they run at once. It never saves over a pending file (the user approves or
//!   rejects it first). An agent's write never touches the app config folder, so it stays pending
//!   until the user approves that exact version, and an agent removes only hooks an agent created.
//! - Each write checks again, right before its rename, that the file still holds the version it
//!   read, and fails with a conflict otherwise. This is best effort, not a compare-and-swap across
//!   processes: a write that lands between the check and the rename is replaced. It fails closed,
//!   since only the exact approved bytes run: the worst case is a lost edit or a pending hook.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::{Map, Value};

use super::approvals::{Approvals, approved_bytes, approved_snapshot, remove_snapshot, save_snapshot};
use super::draft::{check_room, draft_hook};
use super::folder::read_file;
use super::format::{hook_bytes, is_hook_id, parse_hook_file};
use super::sha256_hex;
use super::view::{FileOwner, HookFile, view_of, views};
use super::{HOOKS_FOLDER, hooks_root};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{Hook, HookApproval, HookAuthor, HookDraft, HookListing, HookView};

pub use super::view::{HookCapabilities, MAX_HOOKS_PER_SCOPE};

/// The hooks folder (`<root>/hooks`) and the app config folder that holds the approvals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookStore {
    root: PathBuf,
    config_dir: PathBuf,
}

impl HookStore {
    /// A store for the hooks root `root` and the app config folder `config_dir`.
    pub fn new(root: impl Into<PathBuf>, config_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            config_dir: config_dir.into(),
        }
    }

    /// The store at [`hooks_root`]; `None` when there is no hooks folder (Windows without
    /// `LOCALAPPDATA`).
    pub fn from_env(env: &Env, config_dir: impl Into<PathBuf>) -> Option<Self> {
        hooks_root(env).map(|root| Self::new(root, config_dir))
    }

    /// The hooks root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<root>/hooks`, where the files are.
    pub fn hooks_dir(&self) -> PathBuf {
        self.root.join(HOOKS_FOLDER)
    }

    /// The app config folder (approvals, approved copies and the seed record).
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Every hook file with its status (see [`HookListing`]). A missing folder lists nothing.
    /// Reads the app config folder, never writes it, and never the team copies (see
    /// [`super::team::app_listing`] for the app window's listing with them).
    pub fn list(&self, capabilities: HookCapabilities) -> HookListing {
        self.listing(capabilities, |_| true)
    }

    /// The hook files a chat in `project` (a project root; `None`: global only) sees, as
    /// [`HookStore::list`] shows them: the global hooks, that project's (those over the limit
    /// included), and every file that is not a valid hook, since its scope is unknown.
    pub fn list_for(&self, capabilities: HookCapabilities, project: Option<&str>) -> HookListing {
        self.listing(capabilities, |owner| owner.shown_in(project))
    }

    fn listing(&self, capabilities: HookCapabilities, keep: impl Fn(&FileOwner) -> bool) -> HookListing {
        match self.read_files() {
            Ok(files) => {
                let approvals = self.read_approvals();
                let approved = |id: &str| self.snapshot_of(&approvals, id);

                let hooks = views(files, &approvals, capabilities, &approved)
                    .into_iter()
                    .filter(|(_, owner)| keep(owner))
                    .map(|(view, _)| view)
                    .collect();

                HookListing {
                    hooks,
                    folder_problem: None,
                    team: None,
                }
            }
            Err(problem) => HookListing {
                hooks: Vec::new(),
                folder_problem: Some(problem),
                team: None,
            },
        }
    }

    /// The hook file `id` as [`HookStore::list`] shows it. `NotFound` when there is none,
    /// `Forbidden` when the folder is not read.
    pub fn get(&self, id: &str, capabilities: HookCapabilities) -> Result<HookView> {
        check_id(id)?;

        let listing = self.list(capabilities);

        if let Some(problem) = listing.folder_problem {
            return Err(BusinessError::Forbidden(problem));
        }

        listing
            .hooks
            .into_iter()
            .find(|view| view.id == id)
            .ok_or_else(|| BusinessError::NotFound("the hook".to_owned()))
    }

    /// The approved earlier version of hook `id` (the copy kept when it was approved), while its
    /// approval still names that copy.
    pub fn approved_snapshot(&self, id: &str) -> Option<Hook> {
        self.snapshot_of(&self.read_approvals(), id)
    }

    /// Creates or edits a hook as `author`. The app window's write is approved at once; an
    /// agent's is pending. `InvalidInput` for a draft that breaks the format, a 65th hook in a
    /// scope, an edit without `expectedSha256` or an app window edit of a pending file (approve or
    /// reject it first), `Conflict` when the edited file changed since `expectedSha256`,
    /// `NotFound` for an edit of a missing file or a missing project folder, `Forbidden` when the
    /// folder is writable by other users. The app window's write fails before it changes anything
    /// when the approvals cannot be read.
    pub fn save(
        &self,
        draft: &HookDraft,
        author: HookAuthor,
        capabilities: HookCapabilities,
        now: SystemTime,
    ) -> Result<HookView> {
        self.ensure_dirs()?;

        let files = self.read_files().map_err(BusinessError::Forbidden)?;
        let (id, existing) = self.target_of(draft, &files)?;

        let approvals = match author {
            HookAuthor::Desktop => Approvals::load_for_update(&self.config_dir)?,
            HookAuthor::Agent => self.read_approvals(),
        };

        if author == HookAuthor::Desktop
            && let Some(file) = existing
            && !approvals.is_approved(&file.id, &file.sha256)
        {
            return Err(BusinessError::InvalidInput(
                "the hook is waiting for review: approve or reject it first".to_owned(),
            ));
        }

        let base = existing.map(object_of).unwrap_or_default();
        let previous = existing.and_then(|file| parse_hook_file(&file.id, &file.bytes).ok());

        let hook = draft_hook(draft, &id, author, &base, previous.as_ref(), now)?;
        let bytes = hook_bytes(&hook, base);

        // The same rules as every reader, so no writer saves a file the readers reject.
        parse_hook_file(&id, &bytes).map_err(BusinessError::InvalidInput)?;

        check_room(counted_hooks(&files, author, &approvals).iter(), &hook)?;

        let expected = existing.map(|file| file.sha256.as_str());
        let sha256 = self.write_file(&id, &bytes, expected)?;

        let approvals = match author {
            HookAuthor::Desktop => self.record_approval(approvals, &id, &bytes)?,
            HookAuthor::Agent => approvals,
        };
        let approved = |id: &str| self.snapshot_of(&approvals, id);

        Ok(view_of(hook, sha256, &approvals, capabilities, &approved))
    }

    /// Deletes the hook file `id` as `author`. The app window deletes any file (valid or not) and
    /// drops its approval; `expected_sha256` is optional for it, and a missing file is not an
    /// error. An agent must name the current version (`Conflict` otherwise, `NotFound` for a
    /// missing file) and may remove only a valid hook with `createdBy: "agent"` (`Forbidden`
    /// otherwise: the user decides).
    pub fn delete(&self, id: &str, author: HookAuthor, expected_sha256: Option<&str>) -> Result<()> {
        check_id(id)?;

        let path = self.file_path(id);
        let current = read_file(&path, id);

        if let Some(expected) = expected_sha256
            && current.as_ref().is_some_and(|file| file.sha256 != expected)
        {
            return Err(BusinessError::Conflict("the hook changed since it was read".to_owned()));
        }

        if author == HookAuthor::Agent {
            check_agent_removal(id, current.as_ref(), expected_sha256)?;
        }

        // Read before anything changes: approvals that cannot be read stop the delete.
        let approvals = match author {
            HookAuthor::Desktop => Some(Approvals::load_for_update(&self.config_dir)?),
            HookAuthor::Agent => None,
        };

        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(BusinessError::from_io("deleting a hook", error)),
        }

        if let Some(mut approvals) = approvals {
            if approvals.remove(id) {
                approvals.save(&self.config_dir)?;
            }

            remove_snapshot(&self.config_dir, id);
        }

        Ok(())
    }

    /// The user approves the current bytes of hook `id` (app window only). `Conflict` when they
    /// are not the version the user reviewed (`expected_sha256`), `InvalidInput` for an invalid
    /// file, `NotFound` for a missing one.
    pub fn approve(&self, id: &str, expected_sha256: &str) -> Result<()> {
        check_id(id)?;

        let file =
            read_file(&self.file_path(id), id).ok_or_else(|| BusinessError::NotFound("the hook file".to_owned()))?;

        if file.sha256 != expected_sha256 {
            return Err(BusinessError::Conflict(
                "the hook changed since it was shown".to_owned(),
            ));
        }

        parse_hook_file(id, &file.bytes).map_err(BusinessError::InvalidInput)?;

        let approvals = Approvals::load_for_update(&self.config_dir)?;
        self.record_approval(approvals, id, &file.bytes).map(drop)
    }

    /// The user rejects the pending version of hook `id` (`expected_sha256`, app window only): a
    /// changed hook gets its approved version back (the kept copy, so it is approved again at
    /// once), a never-approved one is deleted. Returns the restored hook's view, `None` after a
    /// delete. `Conflict` when the file changed since it was shown, `NotFound` when there is no
    /// file or no kept copy of the approved version, `InvalidInput` for an approved hook.
    pub fn reject(&self, id: &str, expected_sha256: &str, capabilities: HookCapabilities) -> Result<Option<HookView>> {
        let view = self.get(id, capabilities)?;

        if view.sha256 != expected_sha256 {
            return Err(BusinessError::Conflict(
                "the hook changed since it was shown".to_owned(),
            ));
        }

        match view.approval {
            HookApproval::Approved => Err(BusinessError::InvalidInput("the hook is not pending".to_owned())),
            HookApproval::Never => {
                self.delete(id, HookAuthor::Desktop, Some(expected_sha256))?;
                Ok(None)
            }
            HookApproval::Changed => {
                let approvals = Approvals::read(&self.config_dir)?;
                let bytes = approvals
                    .get(id)
                    .and_then(|sha256| approved_bytes(&self.config_dir, id, sha256))
                    .ok_or_else(|| BusinessError::NotFound("the approved version of the hook".to_owned()))?;

                self.write_file(id, &bytes, Some(expected_sha256))?;
                self.get(id, capabilities).map(Some)
            }
        }
    }

    /// Writes a new hook file and approves it (the Harness family hooks). Returns its hash.
    pub(crate) fn create_approved(&self, hook: &Hook) -> Result<String> {
        self.create_approved_from(hook, Map::new())
    }

    /// Writes a new hook file over `base` (the object it is made from, whose unknown fields it
    /// keeps) and approves it: a team hook the user copies to the local folder. Returns its hash.
    pub(crate) fn create_approved_from(&self, hook: &Hook, base: Map<String, Value>) -> Result<String> {
        self.ensure_dirs()?;

        let files = self.read_files().map_err(BusinessError::Forbidden)?;

        self.write_approved(&files, hook, base, None)
    }

    /// Replaces the file of `hook.id`, while it still hashes to `expected_sha256`, with `hook`
    /// (its unknown fields kept) and approves it, pending or not: the user's explicit request for
    /// a Harness family hook. Returns the new hash.
    pub(crate) fn rewrite_approved(&self, hook: &Hook, expected_sha256: &str) -> Result<String> {
        self.ensure_dirs()?;

        let files = self.read_files().map_err(BusinessError::Forbidden)?;
        let existing = files
            .iter()
            .find(|file| file.id == hook.id)
            .ok_or_else(|| BusinessError::NotFound("the hook file".to_owned()))?;

        if existing.sha256 != expected_sha256 {
            return Err(BusinessError::Conflict("the hook changed since it was read".to_owned()));
        }

        self.write_approved(&files, hook, object_of(existing), Some(expected_sha256))
    }

    /// Writes `hook` (over `base`, the object of the file it replaces) while the file is at
    /// version `expected`, and approves it; `files` is the folder as read. Returns the new hash.
    fn write_approved(
        &self,
        files: &[HookFile],
        hook: &Hook,
        base: Map<String, Value>,
        expected: Option<&str>,
    ) -> Result<String> {
        let bytes = hook_bytes(hook, base);
        parse_hook_file(&hook.id, &bytes).map_err(BusinessError::Internal)?;

        let approvals = Approvals::load_for_update(&self.config_dir)?;
        check_room(counted_hooks(files, HookAuthor::Desktop, &approvals).iter(), hook)?;

        let sha256 = self.write_file(&hook.id, &bytes, expected)?;
        self.record_approval(approvals, &hook.id, &bytes)?;

        Ok(sha256)
    }

    /// Approves `bytes` for `id` in `approvals` (loaded for the update), saves them and keeps a
    /// copy of the bytes. Returns the saved approvals.
    pub(crate) fn record_approval(&self, mut approvals: Approvals, id: &str, bytes: &[u8]) -> Result<Approvals> {
        approvals.insert(id, &sha256_hex(bytes));
        approvals.save(&self.config_dir)?;

        save_snapshot(&self.config_dir, id, bytes);
        Ok(approvals)
    }

    /// The approvals as a reader sees them: none when the file cannot be read (logged).
    pub(crate) fn read_approvals(&self) -> Approvals {
        Approvals::read(&self.config_dir).unwrap_or_else(|error| {
            tracing::warn!(code = ?error.code(), "the hook approvals cannot be read, every hook is pending");
            Approvals::default()
        })
    }

    fn snapshot_of(&self, approvals: &Approvals, id: &str) -> Option<Hook> {
        approved_snapshot(&self.config_dir, id, approvals.get(id)?)
    }

    /// The id a draft writes and the file it replaces (`None` for a new hook).
    fn target_of<'a>(&self, draft: &HookDraft, files: &'a [HookFile]) -> Result<(String, Option<&'a HookFile>)> {
        let Some(id) = &draft.id else {
            return Ok((uuid::Uuid::new_v4().simple().to_string(), None));
        };

        check_id(id)?;

        let existing = files
            .iter()
            .find(|file| &file.id == id)
            .ok_or_else(|| BusinessError::NotFound("the hook file".to_owned()))?;

        let Some(expected) = &draft.expected_sha256 else {
            return Err(BusinessError::InvalidInput(
                "an edit names the version it changes".to_owned(),
            ));
        };

        if &existing.sha256 != expected {
            return Err(BusinessError::Conflict("the hook changed since it was read".to_owned()));
        }

        Ok((id.clone(), Some(existing)))
    }
}

/// The JSON object in `file` (empty when it holds none), so a rewrite keeps its unknown fields.
fn object_of(file: &HookFile) -> Map<String, Value> {
    match serde_json::from_slice::<Value>(&file.bytes) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

/// The hooks a new one counts against: for the app window the approved valid hooks (so files
/// waiting for review cannot fill a scope), for an agent every valid hook.
fn counted_hooks(files: &[HookFile], author: HookAuthor, approvals: &Approvals) -> Vec<Hook> {
    files
        .iter()
        .filter(|file| author == HookAuthor::Agent || approvals.is_approved(&file.id, &file.sha256))
        .filter_map(|file| parse_hook_file(&file.id, &file.bytes).ok())
        .collect()
}

/// An agent removes only the version it read of a valid hook an agent created.
fn check_agent_removal(id: &str, current: Option<&HookFile>, expected_sha256: Option<&str>) -> Result<()> {
    if expected_sha256.is_none() {
        return Err(BusinessError::InvalidInput(
            "a removal names the version it removes".to_owned(),
        ));
    }

    let Some(file) = current else {
        return Err(BusinessError::NotFound("the hook file".to_owned()));
    };

    let created_by_agent = parse_hook_file(id, &file.bytes).is_ok_and(|hook| hook.created_by == HookAuthor::Agent);

    if !created_by_agent {
        return Err(BusinessError::Forbidden(
            "only the user can remove this hook, ask them to do it in Harness Desktop".to_owned(),
        ));
    }

    Ok(())
}

fn check_id(id: &str) -> Result<()> {
    if !is_hook_id(id) {
        return Err(BusinessError::InvalidInput(
            "a hook id is 32 lowercase hex characters".to_owned(),
        ));
    }

    Ok(())
}
