//! One team copy: the envelope `<root>/team/<id>.json` holds `{"version": 1, "id", "repo",
//! "sha256", "author", "createdAt", "updatedAt", "body"}`, the row as the server answered it.
//!
//! A team body is used only when it hashes to the row's `sha256`, follows the kind's team rules
//! ([`DocKind::parse_team`]) and its `project` matches the row's repository ([`checked_doc`]).
//! Sharing and copying rewrite only a body's `project` ([`with_project`]), so every other field
//! (unknown ones included) travels as written.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::TeamRow;
use crate::error::{BusinessError, Result};
use crate::model::HookScope;
use crate::signed_docs::{DocKind, ScopedDoc, json_object, pretty_json};

/// The only envelope version.
const ENVELOPE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    version: u32,
    #[serde(flatten)]
    row: TeamRow,
}

/// The row in the envelope `text` of file `id`, or why it cannot be used.
pub(crate) fn parse_envelope(id: &str, text: &str) -> std::result::Result<TeamRow, String> {
    let envelope: Envelope = serde_json::from_str(text).map_err(|_| "the team copy is not readable".to_owned())?;

    if envelope.version != ENVELOPE_VERSION {
        return Err("the team copy has another version".to_owned());
    }

    if envelope.row.id != id {
        return Err("the team copy's id does not match its file name".to_owned());
    }

    Ok(envelope.row)
}

/// The envelope of `row` as written to the team folder.
pub(crate) fn envelope_bytes<K: DocKind>(row: &TeamRow) -> Result<Vec<u8>> {
    let envelope = Envelope {
        version: ENVELOPE_VERSION,
        row: row.clone(),
    };

    pretty_json(&envelope, &format!("team {}", K::PLURAL))
}

/// The document in `row`, whose body hashes to `sha256` (the caller's hash of it): it must be the
/// row's own hash, follow the kind's team rules, and its `project` must match the row's
/// repository.
pub fn checked_doc<K: DocKind>(row: &TeamRow, sha256: &str) -> std::result::Result<K::Doc, String> {
    if row.sha256 != sha256 {
        return Err("the team copy does not match its hash".to_owned());
    }

    let doc = K::parse_team(&row.id, row.body.as_bytes())?;
    check_repo(row, doc.doc_scope(), doc.doc_project())?;

    Ok(doc)
}

/// A global document's row has no repository, a project document's row the one its `project`
/// names.
pub fn check_repo(row: &TeamRow, scope: HookScope, project: Option<&str>) -> std::result::Result<(), String> {
    let matches = match scope {
        HookScope::Global => row.repo.is_none(),
        HookScope::Project => row.repo.is_some() && row.repo.as_deref() == project,
    };

    if matches {
        Ok(())
    } else {
        Err("the team copy's repository does not match its project".to_owned())
    }
}

/// `bytes` (a document object) with `project` set to `project`, written as the apps write their
/// files.
pub fn with_project<K: DocKind>(bytes: &[u8], project: &str) -> Result<Vec<u8>> {
    let mut map = json_object(bytes);

    if map.is_empty() {
        return Err(BusinessError::InvalidInput(format!(
            "the {} is not a JSON object",
            K::NOUN
        )));
    }

    map.insert("project".to_owned(), Value::from(project));

    pretty_json(&Value::Object(map), &format!("a {}", K::NOUN))
}
