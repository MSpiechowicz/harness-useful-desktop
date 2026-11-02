//! Documents that look like the files about to be imported (`knowledge_similar`), read from
//! `harness-knowledge docs similar --json`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{AuthorRef, KnowledgeId, KnowledgePlace, lenient_author};

/// Why a stored document counts as a version of a file, the strongest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeMatchReason {
    /// The document was imported from this path (or carries this source key).
    SourceKey,
    /// The extracted text is the same.
    SameContent,
    /// More than half of the file's distinct chunks, and at least two, are in the document.
    SharedChunks,
    /// The title is the same in the same repository or global scope.
    Title,
}

/// The stored document a file is probably a version of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeMatch {
    /// The document, to give back as `replacing`.
    pub id: KnowledgeId,
    /// Its title (cleaned of control and invisible characters).
    pub title: String,
    /// Where it is kept.
    pub place: KnowledgePlace,
    /// Why it counts.
    pub reason: KnowledgeMatchReason,
    /// How many of the file's distinct chunks the document has.
    pub shared_chunks: u32,
    /// How many distinct chunks the file has.
    pub chunks: u32,
    /// Who first imported it, none when no name was known.
    #[serde(default, with = "lenient_author")]
    #[ts(optional = nullable, as = "Option<AuthorRef>")]
    pub added_by: Option<AuthorRef>,
    /// When it last changed (RFC 3339).
    pub updated_at: String,
    /// Who last changed it, none when no name was known.
    #[serde(default, with = "lenient_author")]
    #[ts(optional = nullable, as = "Option<AuthorRef>")]
    pub updated_by: Option<AuthorRef>,
}

/// What the comparison found for one file of the selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeSimilarEntry {
    /// The file's path as the server saw it.
    pub path: String,
    /// The document it is probably a version of, none when it looks new.
    #[serde(rename = "match", default)]
    #[ts(optional = nullable)]
    pub found: Option<KnowledgeMatch>,
    /// Why the file could not be compared (redacted), none when it was.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub error: Option<String>,
}

/// The answer of `knowledge_similar`: one entry per file of the selection, a folder giving its
/// files, in the order of the selection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeSimilar {
    /// One entry per file.
    pub items: Vec<KnowledgeSimilarEntry>,
    /// The remote database cannot be reached, so only this computer's documents were compared.
    #[serde(default)]
    pub offline: bool,
}
