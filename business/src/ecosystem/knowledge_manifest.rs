//! The manifest `harness-knowledge docs import --ndjson` reads on stdin ([`ImportManifest`]):
//! what to import, the links and place of every document, the document each file replaces, and
//! whether the documents are stored as drafts or asked for approval. It is checked here first, so
//! the server is never sent what it would refuse.

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use super::catalog::KNOWLEDGE_SIDECAR;
use super::knowledge_docs::check_links;
use crate::error::{BusinessError, Result};
use crate::model::{KnowledgeFeatures, KnowledgeId, KnowledgeImportOptions};

/// The most bytes of the manifest the server reads.
pub const MANIFEST_MAX_BYTES: usize = 4 * 1024 * 1024;

/// The longest note that goes with asking for approval, in characters.
const NOTE_MAX_CHARS: usize = 500;

/// The manifest of an import: what to import and the links of every document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImportManifest {
    /// Files to import, as absolute paths.
    pub paths: Vec<String>,
    /// Folders to import every document of.
    pub folders: Vec<String>,
    /// The repositories every imported document links to.
    pub links: Vec<crate::model::KnowledgeLink>,
    /// The document the one file of this import replaces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replaces: Option<crate::model::KnowledgeId>,
    /// The document each of several files replaces, by the file's path. Left out unless given.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub replacing: BTreeMap<String, crate::model::KnowledgeId>,
    /// Where the documents are kept (`remote` or `local`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<crate::model::KnowledgePlace>,
    /// Store every document as a draft (a pending version for a replaced live document).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub draft: bool,
    /// Replace the document and ask for approval in one step.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ask: bool,
    /// What the reviewers should know about the replacement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl ImportManifest {
    /// The manifest of `paths` and `folders` for a server without `docs-similar` and `docs-drafts`:
    /// [`with_features`] with none of them.
    ///
    /// [`with_features`]: Self::with_features
    pub fn new(paths: Vec<String>, folders: Vec<String>, options: KnowledgeImportOptions) -> Result<Self> {
        Self::with_features(paths, folders, options, KnowledgeFeatures::default())
    }

    /// The manifest of `paths` and `folders` with the options the user chose. `invalidInput` for
    /// links the server would refuse (the same check as setting a document's links), for
    /// `replaces` unless the import is exactly one file, and for `replacing` that is combined with
    /// `replaces`, names a file that is not one of `paths`, a path twice, an id that is no
    /// document's or an id twice. `unsupported` for `replacing` unless `docs_similar` (the server
    /// lists `docs-similar`): an older server would import the file as a new document. The same
    /// for `draft` and `ask` unless `docs_drafts`: an older server would make the document live.
    /// `invalidInput` for `ask` without a replaced document, and for a `note` without `ask`, empty or
    /// over 500 characters.
    pub fn with_features(
        paths: Vec<String>,
        folders: Vec<String>,
        options: KnowledgeImportOptions,
        features: KnowledgeFeatures,
    ) -> Result<Self> {
        check_links(&options.links)?;
        check_drafts(&options, features.docs_drafts)?;

        if options.replaces.as_ref().is_some_and(|id| !id.is_valid()) {
            return Err(BusinessError::InvalidInput("that is not a document id".to_owned()));
        }

        if options.replaces.is_some() && !(paths.len() == 1 && folders.is_empty()) {
            return Err(BusinessError::InvalidInput(
                "a document is replaced by exactly one file".to_owned(),
            ));
        }

        let replacing = replacing_map(&paths, &options, features.docs_similar)?;

        Ok(Self {
            paths,
            folders,
            links: options.links,
            replaces: options.replaces,
            replacing,
            place: options.place,
            draft: options.draft,
            ask: options.ask,
            note: options.note.map(|note| note.trim().to_owned()),
        })
    }

    /// The manifest as the server reads it. `invalidInput` above [`MANIFEST_MAX_BYTES`].
    pub fn to_json(&self) -> Result<Vec<u8>> {
        let bytes = serde_json::to_vec(self).map_err(|error| BusinessError::Internal(error.to_string()))?;

        if bytes.len() > MANIFEST_MAX_BYTES {
            return Err(BusinessError::InvalidInput(
                "that is too many files for one import, split it".to_owned(),
            ));
        }

        Ok(bytes)
    }
}

/// The refusals of `draft`, `ask` and `note` (see [`ImportManifest::with_features`]).
fn check_drafts(options: &KnowledgeImportOptions, docs_drafts: bool) -> Result<()> {
    if (options.draft || options.ask) && !docs_drafts {
        return Err(BusinessError::Unsupported(format!(
            "this {} cannot keep drafts, so update {}",
            KNOWLEDGE_SIDECAR.binary, KNOWLEDGE_SIDECAR.product
        )));
    }

    if options.ask && options.replaces.is_none() && options.replacing.is_empty() {
        return Err(BusinessError::InvalidInput(
            "asking for approval goes with replacing a document".to_owned(),
        ));
    }

    let Some(note) = &options.note else {
        return Ok(());
    };

    let length = note.trim().chars().count();

    if !options.ask || length == 0 || length > NOTE_MAX_CHARS {
        return Err(BusinessError::InvalidInput(format!(
            "a note goes with asking for approval and has 1 to {NOTE_MAX_CHARS} characters"
        )));
    }

    Ok(())
}

/// The `replacing` of the manifest: each file of `options.replacing` with the document it replaces
/// (see [`ImportManifest::with_features`] for what is refused).
fn replacing_map(
    paths: &[String],
    options: &KnowledgeImportOptions,
    docs_similar: bool,
) -> Result<BTreeMap<String, KnowledgeId>> {
    if options.replacing.is_empty() {
        return Ok(BTreeMap::new());
    }

    if !docs_similar {
        return Err(BusinessError::Unsupported(format!(
            "this {} cannot replace documents found by name, so update {}",
            KNOWLEDGE_SIDECAR.binary, KNOWLEDGE_SIDECAR.product
        )));
    }

    if options.replaces.is_some() {
        return Err(BusinessError::InvalidInput(
            "replacing one document and several are not combined".to_owned(),
        ));
    }

    let staged: HashSet<&str> = paths.iter().map(String::as_str).collect();
    let mut ids = HashSet::new();
    let mut map = BTreeMap::new();

    for replacement in &options.replacing {
        if !staged.contains(replacement.path.as_str()) {
            return Err(BusinessError::InvalidInput(
                "a document is replaced by a file of this import".to_owned(),
            ));
        }

        if !replacement.id.is_document() {
            return Err(BusinessError::InvalidInput("that is not a document id".to_owned()));
        }

        if !ids.insert(&replacement.id) || map.insert(replacement.path.clone(), replacement.id.clone()).is_some() {
            return Err(BusinessError::InvalidInput(
                "each document is replaced by one file at most".to_owned(),
            ));
        }
    }

    Ok(map)
}
