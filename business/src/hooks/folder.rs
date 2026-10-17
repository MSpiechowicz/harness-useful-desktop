//! The hooks folder on disk for [`HookStore`]: reading the `<id>.json` files and their entries,
//! creating the folder, and replacing one file while it still holds the version a writer read.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::format::{MAX_HOOK_FILE_BYTES, is_hook_id};
use super::sha256_hex;
use super::store::HookStore;
use super::view::HookFile;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, refuse_link, replace_atomically_if};
use crate::security::SafeReader;

impl HookStore {
    /// `<root>/hooks/<id>.json`.
    pub(super) fn file_path(&self, id: &str) -> PathBuf {
        self.hooks_dir().join(format!("{id}.json"))
    }

    /// The id of every `<id>.json` entry in the folder, whatever its type; `None` when the folder
    /// cannot be listed completely.
    pub(super) fn entry_ids(&self) -> Option<BTreeSet<String>> {
        let mut ids = BTreeSet::new();

        for entry in std::fs::read_dir(self.hooks_dir()).ok()? {
            let name = entry.ok()?.file_name();

            if let Some(id) = name
                .to_str()
                .and_then(|name| name.strip_suffix(".json"))
                .filter(|id| is_hook_id(id))
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
            let current = read_file(&path, id);

            let same = match (expected, current) {
                (None, _) => !exists,
                (Some(expected), Some(file)) => !file.unreadable && file.sha256 == expected,
                (Some(_), None) => false,
            };

            if same {
                Ok(())
            } else {
                Err(BusinessError::Conflict("the hook changed since it was read".to_owned()))
            }
        };

        replace_atomically_if(&path, bytes, "saving a hook", unchanged)?;

        Ok(sha256_hex(bytes))
    }

    /// Creates `<root>` and `<root>/hooks` (`0700` on Unix) when missing; `Forbidden` when the
    /// folder is writable by other users (nothing in it would be read).
    pub(super) fn ensure_dirs(&self) -> Result<()> {
        let dir = self.hooks_dir();
        create_private_dir(&dir, "creating the hooks folder")?;

        let metadata =
            std::fs::metadata(&dir).map_err(|error| BusinessError::from_io("reading the hooks folder", error))?;

        match unsafe_folder(&metadata) {
            Some(problem) => Err(BusinessError::Forbidden(problem.to_owned())),
            None => Ok(()),
        }
    }

    /// Every `<id>.json` regular file, or why the folder is not read.
    pub(super) fn read_files(&self) -> std::result::Result<Vec<HookFile>, String> {
        let dir = self.hooks_dir();

        let Ok(metadata) = std::fs::metadata(&dir) else {
            return Ok(Vec::new());
        };

        if !metadata.is_dir() {
            return Err("the hooks folder is not a folder".to_owned());
        }

        if let Some(problem) = unsafe_folder(&metadata) {
            return Err(problem.to_owned());
        }

        let entries = std::fs::read_dir(&dir).map_err(|_| "the hooks folder could not be read".to_owned())?;
        let mut files = Vec::new();

        for entry in entries.filter_map(|entry| entry.ok()) {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };

            let Some(id) = name.strip_suffix(".json").filter(|id| is_hook_id(id)) else {
                continue;
            };

            if let Some(file) = read_file(&entry.path(), id) {
                files.push(file);
            }
        }

        files.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(files)
    }
}

/// The file at `path` when it is a regular file (not a link). Reads one byte past the limit so an
/// oversized file is recognised without reading all of it.
pub(super) fn read_file(path: &Path, id: &str) -> Option<HookFile> {
    let metadata = std::fs::symlink_metadata(path).ok()?;

    if !metadata.is_file() {
        return None;
    }

    let mut bytes = Vec::new();
    let read = SafeReader::open(path).and_then(|file| {
        file.take(MAX_HOOK_FILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| BusinessError::from_io("reading a hook", error))
    });

    Some(HookFile {
        id: id.to_owned(),
        sha256: sha256_hex(&bytes),
        unreadable: read.is_err(),
        bytes,
    })
}

/// Why a folder with `metadata` must not be trusted (Unix: group or others can write to it).
#[cfg(unix)]
fn unsafe_folder(metadata: &std::fs::Metadata) -> Option<&'static str> {
    use std::os::unix::fs::PermissionsExt;

    (metadata.permissions().mode() & 0o022 != 0).then_some("other users can change the hooks folder")
}

#[cfg(not(unix))]
fn unsafe_folder(_metadata: &std::fs::Metadata) -> Option<&'static str> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A write whose file changed between its read and its rename is refused at the rename.
    #[test]
    fn a_write_checks_the_version_right_before_its_rename() {
        let temp = tempfile::tempdir().unwrap();
        let store = HookStore::new(temp.path().join("root"), temp.path().join("config"));
        store.ensure_dirs().unwrap();

        let id = "0".repeat(32);
        let first = store.write_file(&id, b"first", None).unwrap();

        // Someone else replaced the file after this writer read `first`.
        std::fs::write(store.file_path(&id), b"theirs").unwrap();

        let stale = store.write_file(&id, b"mine", Some(&first));
        assert!(matches!(stale, Err(BusinessError::Conflict(_))), "{stale:?}");

        let created = store.write_file(&id, b"mine", None);
        assert!(matches!(created, Err(BusinessError::Conflict(_))), "{created:?}");

        assert_eq!(std::fs::read(store.file_path(&id)).unwrap(), b"theirs");
        assert_eq!(
            std::fs::read_dir(store.hooks_dir()).unwrap().count(),
            1,
            "no temporary file is left"
        );

        let current = sha256_hex(b"theirs");
        assert_eq!(
            store.write_file(&id, b"mine", Some(&current)).unwrap(),
            sha256_hex(b"mine")
        );
    }
}
