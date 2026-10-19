//! Reading a small versioned JSON file the app window keeps (approvals, state, schedule, records):
//! missing, invalid (too large, not UTF-8, not the expected JSON or version) or valid, and an
//! error when it cannot be read at all, so a writer never saves over a file it could not read.

use std::path::Path;

use serde::de::DeserializeOwned;

use super::approvals::is_regular_file;
use crate::error::{BusinessError, Result};
use crate::security::SafeReader;

/// A file as read.
pub(crate) enum Stored<T> {
    /// There is no file.
    Missing,
    /// A regular file whose content cannot be used.
    Invalid,
    /// The content.
    Valid(T),
}

/// The file at `path` (at most `max_bytes`), `valid` deciding whether its parsed content (the
/// version included) can be used. A regular file that is too large or not UTF-8 is invalid; any
/// other read failure is an error.
pub(crate) fn read_stored<T: DeserializeOwned>(
    path: &Path,
    max_bytes: u64,
    valid: impl FnOnce(&T) -> bool,
) -> Result<Stored<T>> {
    let text = match SafeReader::read_to_string(path, max_bytes) {
        Ok(text) => text,
        Err(BusinessError::NotFound(_)) => return Ok(Stored::Missing),
        Err(BusinessError::InvalidInput(_)) if is_regular_file(path) => return Ok(Stored::Invalid),
        Err(error) => return Err(error),
    };

    match serde_json::from_str::<T>(&text) {
        Ok(value) if valid(&value) => Ok(Stored::Valid(value)),
        _ => Ok(Stored::Invalid),
    }
}
