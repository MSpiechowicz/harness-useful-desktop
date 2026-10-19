//! A kind's folder on disk for [`DocStore`]: reading the `<id>.json` files and their entries,
//! creating the folder, and replacing one file while it still holds the version a writer read.
//!
//! Only names that are exactly `<id>.json` are read, so temp files, leftovers and editor files are
//! ignored, and so is anything that is not a regular file (symbolic links included). On Unix the
//! whole folder is ignored while it or the root is owned by another user or group or others can
//! write to either (a root of the user's own that others can write to loses their write access instead),
//! and a file another user owns is never parsed (it is listed as unreadable).

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::{DocFile, DocKind, DocStore, is_doc_id, sha256_hex};
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, refuse_link, replace_atomically_if};
use crate::security::SafeReader;

impl<K: DocKind> DocStore<K> {
    /// `<root>/<plural>/<id>.json`.
    pub(crate) fn file_path(&self, id: &str) -> PathBuf {
        self.docs_dir().join(format!("{id}.json"))
    }

    /// The id of every `<id>.json` entry in the folder, whatever its type; `None` when the folder
    /// cannot be listed completely.
    pub(crate) fn entry_ids(&self) -> Option<BTreeSet<String>> {
        let mut ids = BTreeSet::new();

        for entry in std::fs::read_dir(self.docs_dir()).ok()? {
            let name = entry.ok()?.file_name();

            if let Some(id) = name
                .to_str()
                .and_then(|name| name.strip_suffix(".json"))
                .filter(|id| is_doc_id(id))
            {
                ids.insert(id.to_owned());
            }
        }

        Some(ids)
    }

    /// Writes `<id>.json` (temp file and rename, `0600`, never through a symbolic link) while the
    /// file still holds the version `expected` (its hash; `None`: no file there), checked again
    /// right before the rename (`Conflict` otherwise). Returns the hash of `bytes`.
    pub(crate) fn write_file(&self, id: &str, bytes: &[u8], expected: Option<&str>) -> Result<String> {
        let path = self.file_path(id);
        refuse_link(&path)?;

        let unchanged = || {
            refuse_link(&path)?;

            let exists = std::fs::symlink_metadata(&path).is_ok();
            let current = read_file::<K>(&path, id);

            let same = match (expected, current) {
                (None, _) => !exists,
                (Some(expected), Some(file)) => !file.unreadable && file.sha256 == expected,
                (Some(_), None) => false,
            };

            if same { Ok(()) } else { Err(changed_since_read::<K>()) }
        };

        replace_atomically_if(&path, bytes, &format!("saving a {}", K::NOUN), unchanged)?;

        Ok(sha256_hex(bytes))
    }

    /// Creates `<root>` and `<root>/<plural>` (`0700` on Unix) when missing; `Forbidden` when the
    /// folder is writable by other users (nothing in it would be read).
    pub(crate) fn ensure_dirs(&self) -> Result<()> {
        let dir = self.docs_dir();
        create_private_dir(&dir, &format!("creating the {} folder", K::PLURAL))?;

        let metadata = std::fs::metadata(&dir)
            .map_err(|error| BusinessError::from_io(format!("reading the {} folder", K::PLURAL), error))?;

        match self.unsafe_place(&metadata) {
            Some(problem) => Err(BusinessError::Forbidden(problem)),
            None => Ok(()),
        }
    }

    /// Why the folder (`metadata`) must not be read: [`unsafe_folder`] of it or of the root.
    pub(crate) fn unsafe_place(&self, metadata: &std::fs::Metadata) -> Option<String> {
        unsafe_folder::<K>(metadata).or_else(|| unsafe_root::<K>(&self.root))
    }

    /// Every `<id>.json` regular file, or why the folder is not read.
    pub(crate) fn read_files(&self) -> std::result::Result<Vec<DocFile>, String> {
        let dir = self.docs_dir();

        let Ok(metadata) = std::fs::metadata(&dir) else {
            return Ok(Vec::new());
        };

        if !metadata.is_dir() {
            return Err(format!("the {} folder is not a folder", K::PLURAL));
        }

        if let Some(problem) = self.unsafe_place(&metadata) {
            return Err(problem);
        }

        let entries = std::fs::read_dir(&dir).map_err(|_| format!("the {} folder could not be read", K::PLURAL))?;
        let mut files = Vec::new();

        for entry in entries.filter_map(|entry| entry.ok()) {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };

            let Some(id) = name.strip_suffix(".json").filter(|id| is_doc_id(id)) else {
                continue;
            };

            if let Some(file) = read_file::<K>(&entry.path(), id) {
                files.push(file);
            }
        }

        files.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(files)
    }
}

/// `Conflict`: the file is no longer the version a writer read.
pub(crate) fn changed_since_read<K: DocKind>() -> BusinessError {
    BusinessError::Conflict(format!("the {} changed since it was read", K::NOUN))
}

/// The file at `path` when it is a regular file (not a link). Reads one byte past the limit so an
/// oversized file is recognised without reading all of it.
pub(crate) fn read_file<K: DocKind>(path: &Path, id: &str) -> Option<DocFile> {
    let metadata = std::fs::symlink_metadata(path).ok()?;

    if !metadata.is_file() {
        return None;
    }

    if !owned_by_current_user(&metadata) {
        return Some(DocFile {
            id: id.to_owned(),
            sha256: sha256_hex(&[]),
            unreadable: true,
            bytes: Vec::new(),
        });
    }

    let mut bytes = Vec::new();
    let read = SafeReader::open(path).and_then(|file| {
        file.take(K::MAX_FILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| BusinessError::from_io(format!("reading a {}", K::NOUN), error))
    });

    Some(DocFile {
        id: id.to_owned(),
        sha256: sha256_hex(&bytes),
        unreadable: read.is_err(),
        bytes,
    })
}

/// Why a folder with `metadata` must not be trusted (Unix: another user owns it, or group or
/// others can write to it).
pub(crate) fn unsafe_folder<K: DocKind>(metadata: &std::fs::Metadata) -> Option<String> {
    if !owned_by_current_user(metadata) {
        return Some(format!("the {} folder belongs to another user", K::PLURAL));
    }

    others_may_write(metadata).then(|| format!("other users can change the {} folder", K::PLURAL))
}

/// Why the kind's root `root` must not be trusted: it cannot be read, another user owns it, or
/// group or others can write to it (Unix), so they could swap the folders inside. A root the user
/// owns that group or others can write to (an older version or a loose umask made it so) loses
/// their write access first ([`tighten_root`]).
pub(crate) fn unsafe_root<K: DocKind>(root: &Path) -> Option<String> {
    tighten_root::<K>(root);

    let Ok(metadata) = std::fs::metadata(root) else {
        return Some(format!("the {} root folder could not be read", K::PLURAL));
    };

    if !owned_by_current_user(&metadata) {
        return Some(format!("the {} root folder belongs to another user", K::PLURAL));
    }

    others_may_write(&metadata).then(|| format!("other users can change the {} root folder", K::PLURAL))
}

/// Unix: takes the group and other write bits off the root folder `root` (and logs it, once per
/// folder) when it is a real folder, not a symbolic link, that the user owns and group or others
/// can write to; its other bits stay. Anything else is left as it is for the checks to refuse: a
/// link (never followed: the folder is opened without following one and changed through that
/// handle), another user's folder, a missing one.
#[cfg(unix)]
fn tighten_root<K: DocKind>(root: &Path) {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let flags = nix::fcntl::OFlag::O_NOFOLLOW | nix::fcntl::OFlag::O_DIRECTORY;
    let Ok(folder) = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags.bits())
        .open(root)
    else {
        return;
    };

    let Ok(metadata) = folder.metadata() else {
        return;
    };

    if !metadata.is_dir() || !owned_by_current_user(&metadata) || !others_may_write(&metadata) {
        return;
    }

    let mode = metadata.permissions().mode() & !0o022;
    let changed = folder.set_permissions(std::fs::Permissions::from_mode(mode));

    // Every listing checks the root: a folder that cannot be changed would log on each poll.
    if !first_time(root) {
        return;
    }

    match changed {
        Ok(()) => tracing::info!(
            kind = K::PLURAL,
            "group and other users can no longer write to the root folder"
        ),
        Err(error) => {
            tracing::warn!(kind = K::PLURAL, error = ?error.kind(), "a root folder other users can change could not be tightened")
        }
    }
}

/// True the first time it is asked about `root` in this process.
#[cfg(unix)]
fn first_time(root: &Path) -> bool {
    static LOGGED: std::sync::Mutex<BTreeSet<PathBuf>> = std::sync::Mutex::new(BTreeSet::new());

    LOGGED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(root.to_path_buf())
}

#[cfg(not(unix))]
fn tighten_root<K: DocKind>(_root: &Path) {}

/// True when the user this app runs as (the effective user id) owns the entry with `metadata`
/// (always on Windows).
#[cfg(unix)]
pub fn owned_by_current_user(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    metadata.uid() == nix::unistd::geteuid().as_raw()
}

#[cfg(not(unix))]
pub fn owned_by_current_user(_metadata: &std::fs::Metadata) -> bool {
    true
}

/// True when group or others can write to a folder with `metadata` (never on Windows).
#[cfg(unix)]
pub(crate) fn others_may_write(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o022 != 0
}

#[cfg(not(unix))]
pub(crate) fn others_may_write(_metadata: &std::fs::Metadata) -> bool {
    false
}
