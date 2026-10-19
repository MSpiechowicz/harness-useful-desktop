//! The app window's own routine files in `<app_local_data_dir>` (the schedule, the run records and
//! the session tags): versioned JSON objects, `0600`, replaced in one rename and never written
//! through a symbolic link, the folders `0700` on Unix.
//!
//! A reader ([`read_json`]) never writes: a missing or invalid file reads as nothing. A writer
//! ([`load_json`]) never saves over a file it could not read; an invalid one (too large, not
//! UTF-8, not the expected JSON or version) is set aside as `<name>.corrupt-<unix-seconds>` and
//! the writer starts again from nothing.

use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::RoutineKind;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, refuse_link, replace_atomically};
use crate::signed_docs::approvals::set_aside;
use crate::signed_docs::json_file::{Stored, read_stored};
use crate::signed_docs::pretty_json;

/// The file at `path`, read only: `None` when it is missing, invalid or cannot be read.
pub(crate) fn read_json<T: DeserializeOwned>(path: &Path, max_bytes: u64, valid: impl FnOnce(&T) -> bool) -> Option<T> {
    match read_stored(path, max_bytes, valid) {
        Ok(Stored::Valid(value)) => Some(value),
        Ok(Stored::Missing | Stored::Invalid) => None,
        Err(error) => {
            tracing::warn!(code = ?error.code(), "a routines file cannot be read");
            None
        }
    }
}

/// The file at `path`, for a writer that saves it again: `None` when it is missing or was invalid
/// (then set aside), an error when it cannot be read (it must not be saved over).
pub(crate) fn load_json<T: DeserializeOwned>(
    path: &Path,
    max_bytes: u64,
    valid: impl FnOnce(&T) -> bool,
) -> Result<Option<T>> {
    match read_stored(path, max_bytes, valid)? {
        Stored::Missing => Ok(None),
        Stored::Invalid => {
            tracing::warn!("a routines file is invalid; set aside and started again");
            set_aside::<RoutineKind>(path)?;
            Ok(None)
        }
        Stored::Valid(value) => Ok(Some(value)),
    }
}

/// Writes `value` to `path` as the apps write their files (the parent folder created `0700` when
/// missing; the file `0600`, one rename, never through a symbolic link). `what` names it in errors.
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T, what: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        refuse_link(parent)?;
        create_private_dir(parent, &format!("creating the folder of {what}"))?;
    }

    refuse_link(path)?;

    let bytes = pretty_json(value, what)?;

    replace_atomically(path, &bytes, &format!("saving {what}"))
}

/// Removes the file at `path`; a missing one is fine.
pub(crate) fn remove_file(path: &Path, what: &str) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(BusinessError::from_io(format!("removing {what}"), error)),
    }
}
