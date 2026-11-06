//! Who wrote a document, note or script: the git `user.name` of the writer and, for an agent's
//! write, the MCP client it came through. Harness Knowledge and Harness Memory print the same
//! `{"name", "via"}` object as `addedBy` and `updatedBy` and `null` when no name was known.
//!
//! The strings are text from another program, shown in the UI: they are cleaned on the way in
//! ([`lenient_author`]) so a name never carries a control or invisible character or an unbounded
//! run of text.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{BusinessError, Result};
use crate::signed_docs::unicode::is_hidden_char;

/// The most characters of a name, as the servers cap it.
pub const MAX_AUTHOR_NAME_CHARS: usize = 80;

/// The most characters of the client name an agent write came through.
pub const MAX_AUTHOR_VIA_CHARS: usize = 64;

/// Who added or last changed a document, note or script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthorRef {
    /// The git `user.name` of the writer, never the email.
    pub name: String,
    /// The MCP client (`claude-code`, `codex`, ...) for an agent's write, none for the user's own.
    #[serde(default)]
    pub via: Option<String>,
}

/// Which writer a list keeps (`--author NAME`, `--author-unknown`). Asked of a server that lists
/// `notes-author-filter` (notes and scripts) or `docs-author-filter` (documents) only.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum AuthorFilter {
    /// What this person first added: the exact git `user.name` of the writer.
    Name {
        /// The name as the facet lists it.
        name: String,
    },
    /// What has no known writer.
    Unknown,
}

impl AuthorFilter {
    /// This filter when the servers would take it, else `invalidInput`: a name is what a server
    /// would store (see [`is_clean_author_name`]). A name the servers refuse exits with a usage
    /// error, which reads as an older server, so it is never sent.
    pub fn checked(&self) -> Result<&Self> {
        match self {
            Self::Name { name } if !is_clean_author_name(name) => Err(BusinessError::InvalidInput(format!(
                "an author name is 1 to {MAX_AUTHOR_NAME_CHARS} characters without control or invisible ones"
            ))),
            _ => Ok(self),
        }
    }
}

/// One person in the author facet of a list and how many entries they first added under the other
/// filters of the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthorCount {
    /// The git `user.name`, cleaned (see [`is_clean_author_name`]).
    pub name: String,
    /// How many entries they added.
    pub count: u32,
}

/// Who added the entries of a list, under the list's other filters (not the author filter itself,
/// so the choices stay). `None` on a page from a server that does not send it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthorFacet {
    /// The writers, in the server's order (at most 201).
    pub people: Vec<AuthorCount>,
    /// How many entries have no known writer.
    pub unknown: u32,
}

/// Whether `name` is an author name as the servers store it: not empty, at most
/// [`MAX_AUTHOR_NAME_CHARS`] characters, trimmed, with no control or invisible character, so
/// cleaning it ([`display_text`]) changes nothing.
pub fn is_clean_author_name(name: &str) -> bool {
    !name.is_empty() && display_text(name, MAX_AUTHOR_NAME_CHARS) == name
}

/// The most characters of a server's reason for a database it cannot reach that are shown.
pub const MAX_OFFLINE_REASON_CHARS: usize = 300;

/// A server's `offlineReason` as it may be shown: cleaned and bounded, none when blank.
pub fn offline_reason_text(reason: &str) -> Option<String> {
    let text = display_text(reason, MAX_OFFLINE_REASON_CHARS);

    (!text.is_empty()).then_some(text)
}

/// `text` without control characters and characters that draw nothing or reorder the text around
/// them, trimmed, at most `max_chars` long. For text of another program that is shown as it is.
pub fn display_text(text: &str, max_chars: usize) -> String {
    let kept: String = text
        .trim()
        .chars()
        .filter(|character| !character.is_control() && !is_hidden_char(*character))
        .take(max_chars)
        .collect();

    kept.trim().to_owned()
}

/// The serde `with` of the `addedBy` / `updatedBy` fields (`#[serde(default, with =
/// "lenient_author")]`): the object with its name cleaned (see the module docs), and `None` for
/// `null`, an absent key, anything that is not such an object and a name that is empty once
/// cleaned. An author this app cannot read never fails the list around it.
pub mod lenient_author {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde_json::Value;

    use super::{AuthorRef, MAX_AUTHOR_NAME_CHARS, MAX_AUTHOR_VIA_CHARS, display_text};

    /// Writes the author as it is.
    pub fn serialize<S: Serializer>(author: &Option<AuthorRef>, serializer: S) -> Result<S::Ok, S::Error> {
        author.serialize(serializer)
    }

    /// Reads and cleans the author (see the module docs).
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<AuthorRef>, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            name: String,
            via: Option<String>,
        }

        let Some(value) = Option::<Value>::deserialize(deserializer)? else {
            return Ok(None);
        };

        let Ok(raw) = serde_json::from_value::<Raw>(value) else {
            return Ok(None);
        };

        let name = display_text(&raw.name, MAX_AUTHOR_NAME_CHARS);

        if name.is_empty() {
            return Ok(None);
        }

        let via = raw
            .via
            .map(|via| display_text(&via, MAX_AUTHOR_VIA_CHARS))
            .filter(|via| !via.is_empty());

        Ok(Some(AuthorRef { name, via }))
    }
}

/// The serde `with` of a teammate's name that is a plain optional string (`#[serde(default, with =
/// "lenient_name")]`): cleaned like an [`AuthorRef`]'s name ([`display_text`], at most
/// [`MAX_AUTHOR_NAME_CHARS`]), and `None` for `null`, anything that is not a string and a name that
/// is empty once cleaned.
pub mod lenient_name {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde_json::Value;

    use super::{MAX_AUTHOR_NAME_CHARS, display_text};

    /// Writes the name as it is.
    pub fn serialize<S: Serializer>(name: &Option<String>, serializer: S) -> Result<S::Ok, S::Error> {
        name.serialize(serializer)
    }

    /// Reads and cleans the name (see the module docs).
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
        let value = Option::<Value>::deserialize(deserializer)?;

        Ok(match value {
            Some(Value::String(name)) => {
                Some(display_text(&name, MAX_AUTHOR_NAME_CHARS)).filter(|name| !name.is_empty())
            }
            _ => None,
        })
    }
}

/// The serde `with` of a list of teammates' names (`#[serde(default, with = "lenient_names")]`):
/// every name cleaned like [`lenient_name`]'s, the ones that are not strings or are empty once
/// cleaned dropped, and `None` for `null`, an absent key and anything that is not a list. A list
/// this app cannot read never fails the document around it.
pub mod lenient_names {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde_json::Value;

    use super::{MAX_AUTHOR_NAME_CHARS, display_text};

    /// Writes the names as they are.
    pub fn serialize<S: Serializer>(names: &Option<Vec<String>>, serializer: S) -> Result<S::Ok, S::Error> {
        names.serialize(serializer)
    }

    /// Reads and cleans the names (see the module docs).
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Vec<String>>, D::Error> {
        let value = Option::<Value>::deserialize(deserializer)?;

        let Some(Value::Array(items)) = value else {
            return Ok(None);
        };

        let names = items
            .into_iter()
            .filter_map(|item| match item {
                Value::String(name) => Some(display_text(&name, MAX_AUTHOR_NAME_CHARS)),
                _ => None,
            })
            .filter(|name| !name.is_empty())
            .collect();

        Ok(Some(names))
    }
}
