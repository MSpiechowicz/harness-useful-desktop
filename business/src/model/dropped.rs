//! Files attached to a message: dropped onto the window (`drop_listen`) or saved by the shell
//! (`attachment_save`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// An image file the user dropped, read by the shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DroppedImage {
    /// File name without its directory.
    pub name: String,
    /// One of the accepted image types, taken from the file's content, not its name.
    pub mime_type: String,
    /// Base64-encoded file bytes.
    pub data: String,
}

/// A file the agent reads from disk: the message names its path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AttachedFile {
    /// File name without its directory.
    pub name: String,
    /// Absolute path of the file.
    pub path: String,
    /// Size in bytes.
    #[ts(type = "number")]
    pub bytes: u64,
}

/// What the native drag and drop machinery tells the webview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum DropEvent {
    /// Files are being dragged over the window (`true`) or left it (`false`).
    Hover {
        /// Whether a drag is over the window.
        active: bool,
    },
    /// Files were dropped.
    Dropped {
        /// The dropped images, read by the shell, up to the per-message limit.
        images: Vec<DroppedImage>,
        /// Every other dropped file (and images beyond the limit or too large to send): named by path.
        files: Vec<AttachedFile>,
        /// Names of dropped items that were left out (folders, unreadable files).
        skipped: Vec<String>,
    },
}
