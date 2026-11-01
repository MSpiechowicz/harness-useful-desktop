//! Which user MCP files the app registered the knowledge server in (`knowledge-registration.json`
//! in the app's config folder), so a server the user removed is not added back on the next launch.
//! The file, its limits and the rebuilding of one that cannot be used are
//! [`sidecar_record`](super::sidecar_record)'s. Turning project knowledge on clears the record, so
//! the server is added back everywhere.

use super::sidecar_record::{RecordFile, SidecarRecord};

/// File name inside the app's config folder.
pub const KNOWLEDGE_RECORD_FILE: &str = "knowledge-registration.json";

/// The knowledge server's record.
#[derive(Debug)]
pub struct KnowledgeFile;

impl RecordFile for KnowledgeFile {
    const FILE: &'static str = KNOWLEDGE_RECORD_FILE;
    const PRODUCT: &'static str = "knowledge";
}

/// The record of one config folder, as loaded.
pub type KnowledgeRecord = SidecarRecord<KnowledgeFile>;
