//! The last transcripts read, by session file, size and modification time, so paging through a
//! long session parses its file once rather than once per page. Bounded to
//! [`MAX_CACHED_TRANSCRIPTS`] files of at most [`MAX_CACHED_BYTES`] together (by file size), the
//! least recently used dropped first. A file larger than that bound on its own is not cached.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use crate::error::Result;
use crate::model::ChatMessage;

/// Transcripts kept at most.
const MAX_CACHED_TRANSCRIPTS: usize = 4;

/// Total size of the session files whose transcripts are kept, at most (64 MiB).
const MAX_CACHED_BYTES: u64 = 64 * 1024 * 1024;

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

/// Cached transcripts, most recently used first, within a byte budget.
struct Cache {
    entries: Vec<Entry>,
    max_bytes: u64,
}

impl Cache {
    const fn new(max_bytes: u64) -> Self {
        Self {
            entries: Vec::new(),
            max_bytes,
        }
    }

    /// The cached transcript of `path` read at `stamp`, moved to the front.
    fn hit(&mut self, path: &Path, stamp: &Stamp) -> Option<Arc<Vec<ChatMessage>>> {
        let position = self
            .entries
            .iter()
            .position(|entry| entry.path == path && &entry.stamp == stamp)?;

        let entry = self.entries.remove(position);
        let messages = Arc::clone(&entry.messages);
        self.entries.insert(0, entry);

        Some(messages)
    }

    /// Keeps `messages` of `path` at the front, dropping the least recently used transcripts past
    /// the count or byte bound. A file over the byte bound on its own is not kept.
    fn insert(&mut self, path: &Path, stamp: Stamp, messages: &Arc<Vec<ChatMessage>>) {
        self.entries.retain(|entry| entry.path != path);

        if stamp.size > self.max_bytes {
            return;
        }

        self.entries.insert(
            0,
            Entry {
                path: path.to_path_buf(),
                stamp,
                messages: Arc::clone(messages),
            },
        );

        let mut total = 0_u64;
        let kept = self
            .entries
            .iter()
            .take(MAX_CACHED_TRANSCRIPTS)
            .take_while(|entry| {
                total = total.saturating_add(entry.stamp.size);
                total <= self.max_bytes
            })
            .count();
        self.entries.truncate(kept);
    }
}

static CACHE: Mutex<Cache> = Mutex::new(Cache::new(MAX_CACHED_BYTES));

/// The cached transcript of `path` while its size and modification time match, else `read`'s
/// (then cached). A file whose metadata cannot be read is not cached.
pub(super) fn cached(path: &Path, read: impl FnOnce() -> Result<Vec<ChatMessage>>) -> Result<Arc<Vec<ChatMessage>>> {
    cached_in(&CACHE, path, read)
}

fn cached_in(
    cache: &Mutex<Cache>,
    path: &Path,
    read: impl FnOnce() -> Result<Vec<ChatMessage>>,
) -> Result<Arc<Vec<ChatMessage>>> {
    let Some(stamp) = stamp(path) else {
        return read().map(Arc::new);
    };

    let hit = cache.lock().unwrap_or_else(PoisonError::into_inner).hit(path, &stamp);
    if let Some(messages) = hit {
        return Ok(messages);
    }

    let messages = Arc::new(read()?);

    cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(path, stamp, &messages);

    Ok(messages)
}

fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = fs::metadata(path).ok()?;

    Some(Stamp {
        size: metadata.len(),
        modified: metadata.modified().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    /// A session file of `size` bytes in `dir`.
    fn file(dir: &Path, name: &str, size: u64) -> PathBuf {
        let path = dir.join(name);
        fs::File::create(&path).unwrap().set_len(size).unwrap();
        path
    }

    /// Reads `path` through `cache` and returns whether the file was parsed again.
    fn parsed(cache: &Mutex<Cache>, path: &Path) -> bool {
        let read = Cell::new(false);

        cached_in(cache, path, || {
            read.set(true);
            Ok(Vec::new())
        })
        .unwrap();

        read.get()
    }

    #[test]
    fn transcripts_are_kept_within_the_byte_budget() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Mutex::new(Cache::new(100));
        let first = file(dir.path(), "first.jsonl", 60);
        let second = file(dir.path(), "second.jsonl", 30);
        let third = file(dir.path(), "third.jsonl", 30);

        assert!(parsed(&cache, &first));
        assert!(!parsed(&cache, &first), "paging again reuses the transcript");

        assert!(parsed(&cache, &second));
        assert!(parsed(&cache, &third), "60 + 30 + 30 bytes is over the budget");
        assert!(!parsed(&cache, &second) && !parsed(&cache, &third));
        assert!(parsed(&cache, &first), "the least recently used was dropped");
    }

    #[test]
    fn a_file_over_the_budget_is_never_kept() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Mutex::new(Cache::new(100));
        let small = file(dir.path(), "small.jsonl", 10);
        let huge = file(dir.path(), "huge.jsonl", 101);

        assert!(parsed(&cache, &small));
        assert!(parsed(&cache, &huge));
        assert!(parsed(&cache, &huge), "parsed for every page");
        assert!(!parsed(&cache, &small), "the cached transcripts stay");
    }

    #[test]
    fn at_most_four_transcripts_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Mutex::new(Cache::new(100));
        let files: Vec<PathBuf> = (0..5).map(|n| file(dir.path(), &format!("{n}.jsonl"), 1)).collect();

        for path in &files {
            assert!(parsed(&cache, path));
        }

        assert!(!parsed(&cache, &files[4]));
        assert!(parsed(&cache, &files[0]));
    }
}
