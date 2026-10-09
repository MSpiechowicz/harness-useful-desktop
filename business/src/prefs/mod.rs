//! User preferences persisted as JSON in the OS app-config directory.

mod store;

pub use store::{MAX_RECENT_PROJECTS, PREFS_FILE, PrefsStore, normalize};
