//! User preferences persisted as JSON in the OS app-config directory.

mod managed;
mod store;

pub use managed::with_ui_fields;
pub use store::{MAX_RECENT_PROJECTS, PREFS_FILE, PrefsStore, normalize};
