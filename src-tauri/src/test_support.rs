//! Test helpers for the shell's unit tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A uniquely named directory under the OS temp dir, removed on drop. Its path is canonical, so
/// containment checks compare like with like (macOS `/var` → `/private/var`).
pub struct TempDir(PathBuf);

impl TempDir {
    /// Creates `harness-desktop-<label>-<pid>-<n>`.
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);

        let name = format!(
            "harness-desktop-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path).expect("create temp dir");

        Self(std::fs::canonicalize(&path).expect("canonical temp dir"))
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}
