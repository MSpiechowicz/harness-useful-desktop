//! Files the webview hands over as bytes (a picked file, pasted text, a browser drop): saved
//! under an app-owned directory so the agent can open them by path.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use uuid::Uuid;

use crate::error::{BusinessError, Result};
use crate::model::AttachedFile;

/// Largest saved file, in bytes (10 MiB). Larger files are dropped natively, which names the
/// original path instead of copying.
pub const MAX_SAVED_BYTES: usize = 10 * 1024 * 1024;

/// How long a saved attachment is kept.
pub const KEEP: Duration = Duration::from_secs(14 * 24 * 60 * 60);

const MAX_NAME_CHARS: usize = 100;

/// Saves base64 `data` as `name` in a fresh directory under `root` and returns it. The name is
/// reduced to a safe file name; the directory is the app's, never one the webview chose.
pub fn save_attachment(root: &Path, name: &str, data: &str) -> Result<AttachedFile> {
    if data.len() > MAX_SAVED_BYTES.div_ceil(3) * 4 {
        return Err(BusinessError::InvalidInput(format!(
            "a file holds at most {MAX_SAVED_BYTES} bytes"
        )));
    }

    let bytes = STANDARD
        .decode(data)
        .map_err(|_| BusinessError::InvalidInput("the file is not valid base64".to_owned()))?;
    if bytes.len() > MAX_SAVED_BYTES {
        return Err(BusinessError::InvalidInput(format!(
            "a file holds at most {MAX_SAVED_BYTES} bytes"
        )));
    }

    prune(root, SystemTime::now());

    let name = safe_name(name);
    let directory = root.join(Uuid::new_v4().to_string());
    fs::create_dir_all(&directory)
        .map_err(|error| BusinessError::from_io("cannot create the attachment folder", error))?;
    let path = directory.join(&name);
    fs::write(&path, &bytes).map_err(|error| BusinessError::from_io("cannot save the attachment", error))?;

    Ok(AttachedFile {
        name,
        path: path.to_string_lossy().into_owned(),
        bytes: bytes.len() as u64,
    })
}

/// A file name without directories, control characters or a leading dot; `attachment` when nothing is left.
pub fn safe_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = last
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.');

    if trimmed.is_empty() {
        "attachment".to_owned()
    } else {
        trimmed.chars().take(MAX_NAME_CHARS).collect()
    }
}

/// Removes saved attachments older than [`KEEP`]; failures are ignored (it is housekeeping).
pub fn prune(root: &Path, now: SystemTime) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| now.duration_since(modified).is_ok_and(|age| age > KEEP));

        if old {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lose_directories_and_odd_characters() {
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_name("C:\\logs\\run: 1?.txt"), "run_ 1_.txt");
        assert_eq!(safe_name(".bashrc"), "bashrc");
        assert_eq!(safe_name("  "), "attachment");
        assert_eq!(safe_name(&"a".repeat(300)).len(), 100);
    }

    #[test]
    fn saves_under_its_own_directory() {
        let root = tempfile::tempdir().unwrap();

        let saved = save_attachment(root.path(), "../build.log", &STANDARD.encode("boom")).unwrap();

        assert_eq!((saved.name.as_str(), saved.bytes), ("build.log", 4));
        assert!(Path::new(&saved.path).starts_with(root.path()));
        assert_eq!(fs::read_to_string(&saved.path).unwrap(), "boom");
    }

    #[test]
    fn refuses_bad_base64_and_oversized_files() {
        let root = tempfile::tempdir().unwrap();

        assert!(save_attachment(root.path(), "a", "not base64!").is_err());
        assert!(save_attachment(root.path(), "a", &STANDARD.encode(vec![0u8; MAX_SAVED_BYTES + 1])).is_err());
    }

    #[test]
    fn prune_removes_only_old_directories() {
        let root = tempfile::tempdir().unwrap();
        let saved = save_attachment(root.path(), "a.txt", "aGk=").unwrap();

        prune(root.path(), SystemTime::now());
        assert!(Path::new(&saved.path).exists());

        prune(root.path(), SystemTime::now() + KEEP + Duration::from_secs(60));
        assert!(!Path::new(&saved.path).exists());
    }
}
