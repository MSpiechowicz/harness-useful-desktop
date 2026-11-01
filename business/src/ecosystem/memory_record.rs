//! Which user MCP files the app registered the memory server in (`memory-registration.json` in the
//! app's config folder), so a server the user removed is not added back on the next launch. The
//! file, its limits and the rebuilding of one that cannot be used are
//! [`sidecar_record`](super::sidecar_record)'s. Turning project memory on clears the record, so the
//! server is added back everywhere.

use super::sidecar_record::{RecordFile, SidecarRecord};

pub use super::sidecar_record::RecordedFile;

/// File name inside the app's config folder.
pub const MEMORY_RECORD_FILE: &str = "memory-registration.json";

/// The memory server's record.
#[derive(Debug)]
pub struct MemoryFile;

impl RecordFile for MemoryFile {
    const FILE: &'static str = MEMORY_RECORD_FILE;
    const PRODUCT: &'static str = "memory";
}

/// The record of one config folder, as loaded.
pub type MemoryRecord = SidecarRecord<MemoryFile>;
