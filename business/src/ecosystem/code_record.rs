//! Which user MCP files the app registered the code server in (`code-registration.json` in the
//! app's config folder), so a server the user removed is not added back on the next launch. The
//! file, its limits and the rebuilding of one that cannot be used are
//! [`sidecar_record`](super::sidecar_record)'s. Turning the code graph on clears the record, so the
//! server is added back everywhere.

use super::sidecar_record::{RecordFile, SidecarRecord};

/// File name inside the app's config folder.
pub const CODE_RECORD_FILE: &str = "code-registration.json";

/// The code server's record.
#[derive(Debug)]
pub struct CodeFile;

impl RecordFile for CodeFile {
    const FILE: &'static str = CODE_RECORD_FILE;
    const PRODUCT: &'static str = "code";
}

/// The record of one config folder, as loaded.
pub type CodeRecord = SidecarRecord<CodeFile>;
