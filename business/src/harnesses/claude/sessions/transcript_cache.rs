//! The last transcripts read, by session file, size and modification time, so paging through a
//! long session parses its file once rather than once per page. Bounded to
//! [`MAX_CACHED_TRANSCRIPTS`] files, the least recently used dropped first.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use crate::error::Result;
use crate::model::ChatMessage;

/// Transcripts kept at most.
const MAX_CACHED_TRANSCRIPTS: usize = 4;

/// What a transcript was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    size: u64,
    modified: SystemTime,
}

/// One cached transcript.
struct Entry {
    path: PathBuf,
    stamp: Stamp,
    messages: Arc<Vec<ChatMessage>>,
}

/// Most recently used first.
static CACHE: Mutex<Vec<Entry>> = Mutex::new(Vec::new());

/// The cached transcript of `path` while its size and modification time match, else `read`'s
/// (then cached). A file whose metadata cannot be read is not cached.
pub(super) fn cached(path: &Path, read: impl FnOnce() -> Result<Vec<ChatMessage>>) -> Result<Arc<Vec<ChatMessage>>> {
    let Some(stamp) = stamp(path) else {
        return read().map(Arc::new);
    };

    if let Some(messages) = hit(path, &stamp) {
        return Ok(messages);
    }

    let messages = Arc::new(read()?);

    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    cache.retain(|entry| entry.path != path);
    cache.insert(
        0,
        Entry {
            path: path.to_path_buf(),
            stamp,
            messages: Arc::clone(&messages),
        },
    );
    cache.truncate(MAX_CACHED_TRANSCRIPTS);

    Ok(messages)
}

/// The cached transcript of `path` read at `stamp`, moved to the front.
fn hit(path: &Path, stamp: &Stamp) -> Option<Arc<Vec<ChatMessage>>> {
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    let position = cache
        .iter()
        .position(|entry| entry.path == path && &entry.stamp == stamp)?;

    let entry = cache.remove(position);
    let messages = Arc::clone(&entry.messages);
    cache.insert(0, entry);

    Some(messages)
}

fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = fs::metadata(path).ok()?;

    Some(Stamp {
        size: metadata.len(),
        modified: metadata.modified().ok()?,
    })
}
