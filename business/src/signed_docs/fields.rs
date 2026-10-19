//! The field rules every kind of signed document shares (see the hook object in
//! `docs/hooks-format.md`): the file as a JSON object, `version`, `id`, `name`, `description`,
//! `scope` and `project`, the RFC 3339 times and the authors. Each kind parses its own fields
//! with these and names the first problem found.

use std::path::Path;

use serde_json::{Map, Value};

use super::team::REPO_KEY_PREFIX;
use super::unicode::is_hidden_char;
use super::{DocKind, is_doc_id};
use crate::model::{HookAuthor, HookScope};

/// Longest `name`, in characters.
pub const MAX_NAME_CHARS: usize = 80;

/// Longest `description`, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 500;

/// Longest repository key of a team document's `project`, in bytes.
pub const MAX_REPO_KEY_BYTES: usize = 512;

/// Longest local project root in a file, in bytes.
pub const MAX_PROJECT_PATH_BYTES: usize = 4096;

/// The only file version.
pub(crate) const VERSION: u64 = 1;

/// What a project document's `project` holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectForm {
    /// A local file: the absolute project root.
    Root,
    /// A team body: the repository key (`git:host/owner/repo`).
    RepoKey,
}

/// The JSON object in `bytes`, or why the file is not one (over [`DocKind::MAX_FILE_BYTES`], not
/// UTF-8, not JSON).
pub(crate) fn file_object<K: DocKind>(bytes: &[u8]) -> Result<Map<String, Value>, String> {
    if bytes.len() > K::MAX_FILE_BYTES {
        return Err(format!("the file is larger than {} KiB", K::MAX_FILE_BYTES / 1024));
    }

    let text = std::str::from_utf8(bytes).map_err(|_| "the file is not UTF-8".to_owned())?;
    let value: Value = serde_json::from_str(text).map_err(|_| "the file is not JSON".to_owned())?;

    match value {
        Value::Object(map) => Ok(map),
        _ => Err("the file is not a JSON object".to_owned()),
    }
}

/// `version` is the number 1.
pub(crate) fn check_version(map: &Map<String, Value>) -> Result<(), String> {
    match map.get("version") {
        None => Err("version is missing".to_owned()),
        Some(version) if version.as_u64() != Some(VERSION) => Err("version must be 1".to_owned()),
        Some(_) => Ok(()),
    }
}

/// `id`: 32 lowercase hex characters, equal to the file name stem `stem`.
pub(crate) fn required_id<'a>(map: &'a Map<String, Value>, stem: &str) -> Result<&'a str, String> {
    let id = required_str(map, "id")?;

    if !is_doc_id(id) {
        return Err("id must be 32 lowercase hex characters".to_owned());
    }

    if id != stem {
        return Err("id does not match the file name".to_owned());
    }

    Ok(id)
}

/// The fields every document starts with, checked in this order: `version`, `id`, `name`,
/// `description`, `scope` and `project`.
pub(crate) struct Head {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub scope: HookScope,
    pub project: Option<String>,
}

/// The head of the document object `map` of file `stem`, `project` in `form`.
pub(crate) fn parse_head<K: DocKind>(map: &Map<String, Value>, stem: &str, form: ProjectForm) -> Result<Head, String> {
    check_version(map)?;

    let id = required_id(map, stem)?.to_owned();

    let name = required_str(map, "name")?;
    check_name(name)?;

    let description = optional_str(map, "description")?;
    if let Some(description) = description {
        check_description(description)?;
    }

    let (scope, project) = parse_scope::<K>(map, form)?;

    Ok(Head {
        id,
        name: name.to_owned(),
        description: description.map(str::to_owned),
        scope,
        project,
    })
}

/// When and by whom a document was created and last written.
pub(crate) struct Stamps {
    pub created_at: String,
    pub created_by: HookAuthor,
    pub updated_at: String,
    pub updated_by: HookAuthor,
}

/// `createdAt`, `createdBy`, `updatedAt` and `updatedBy` of `map`.
pub(crate) fn parse_stamps(map: &Map<String, Value>) -> Result<Stamps, String> {
    Ok(Stamps {
        created_at: required_timestamp(map, "createdAt")?,
        created_by: required_author(map, "createdBy")?,
        updated_at: required_timestamp(map, "updatedAt")?,
        updated_by: required_author(map, "updatedBy")?,
    })
}

/// The object `value` of field `field` (`action`, `when`) and its required `type`.
pub(crate) fn typed_object<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<(&'a Map<String, Value>, &'a str), String> {
    let Some(value) = value else {
        return Err(format!("{field} is missing"));
    };

    let Value::Object(object) = value else {
        return Err(format!("{field} must be an object"));
    };

    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field}.type is missing"))?;

    Ok((object, kind))
}

/// A `command` action as written: `type`, `command` and `timeoutSecs` when set.
pub(crate) fn command_action_value(command: &str, timeout_secs: Option<u32>) -> Value {
    let mut map = Map::new();
    map.insert("type".to_owned(), Value::from("command"));
    map.insert("command".to_owned(), Value::from(command));

    if let Some(secs) = timeout_secs {
        map.insert("timeoutSecs".to_owned(), Value::from(secs));
    }

    Value::Object(map)
}

/// The `name` of an invalid file, when it is a string (for the list).
pub(crate) fn loose_name(bytes: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let name = value.get("name")?.as_str()?;

    Some(name.chars().take(MAX_NAME_CHARS).collect())
}

/// 1 to [`MAX_NAME_CHARS`] characters, not blank, no control or invisible characters
/// ([`is_hidden_char`]).
pub(crate) fn check_name(name: &str) -> Result<(), String> {
    let chars = name.chars().count();

    if chars == 0 || name.trim().is_empty() {
        return Err("name is empty".to_owned());
    }

    if chars > MAX_NAME_CHARS {
        return Err(format!("name is longer than {MAX_NAME_CHARS} characters"));
    }

    if name.chars().any(char::is_control) {
        return Err("name contains a control character".to_owned());
    }

    if name.chars().any(is_hidden_char) {
        return Err("name contains an invisible character".to_owned());
    }

    Ok(())
}

/// At most [`MAX_DESCRIPTION_CHARS`] characters, no invisible characters ([`is_hidden_char`]).
pub(crate) fn check_description(description: &str) -> Result<(), String> {
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!("description is longer than {MAX_DESCRIPTION_CHARS} characters"));
    }

    if description.chars().any(is_hidden_char) {
        return Err("description contains an invisible character".to_owned());
    }

    Ok(())
}

/// `scope` and `project`: a global document has none, a project document's is a root or a
/// repository key as `form` says.
pub(crate) fn parse_scope<K: DocKind>(
    map: &Map<String, Value>,
    form: ProjectForm,
) -> Result<(HookScope, Option<String>), String> {
    let scope = required_str(map, "scope")?;
    let project = optional_str(map, "project")?;

    match (scope, project) {
        ("global", None) => Ok((HookScope::Global, None)),
        ("global", Some(_)) => Err(format!("a global {} has no project", K::NOUN)),
        ("project", None) => Err(format!("a project {} needs a project", K::NOUN)),
        ("project", Some(project)) => {
            check_project(project, form)?;
            Ok((HookScope::Project, Some(project.to_owned())))
        }
        (other, _) => Err(format!("unknown scope {other:?}")),
    }
}

/// A project root is an absolute path (on Windows with its drive or share) of at most
/// [`MAX_PROJECT_PATH_BYTES`] without control or invisible characters; a repository key starts
/// with `git:`, is at most [`MAX_REPO_KEY_BYTES`] long and has no spaces, control or invisible
/// characters.
fn check_project(project: &str, form: ProjectForm) -> Result<(), String> {
    match form {
        ProjectForm::Root if !Path::new(project).is_absolute() => Err("project must be an absolute path".to_owned()),
        ProjectForm::Root if project.len() > MAX_PROJECT_PATH_BYTES => {
            Err(format!("project is longer than {MAX_PROJECT_PATH_BYTES} bytes"))
        }
        ProjectForm::Root
            if project
                .chars()
                .any(|character| character.is_control() || is_hidden_char(character)) =>
        {
            Err("project contains control or invisible characters".to_owned())
        }
        ProjectForm::Root => Ok(()),
        ProjectForm::RepoKey => {
            let plain = project.len() <= MAX_REPO_KEY_BYTES
                && project.len() > REPO_KEY_PREFIX.len()
                && project.starts_with(REPO_KEY_PREFIX)
                && !project
                    .chars()
                    .any(|character| character.is_whitespace() || character.is_control() || is_hidden_char(character));

            if plain {
                Ok(())
            } else {
                Err("project must be a git: repository key".to_owned())
            }
        }
    }
}

/// 1 to `max` characters.
pub(crate) fn check_length(field: &str, text: &str, max: usize) -> Result<(), String> {
    let chars = text.chars().count();

    if chars == 0 {
        return Err(format!("{field} is empty"));
    }

    if chars > max {
        return Err(format!("{field} is longer than {max} characters"));
    }

    Ok(())
}

/// A required `true` or `false`.
pub(crate) fn required_bool(map: &Map<String, Value>, key: &str) -> Result<bool, String> {
    match map.get(key) {
        None => Err(format!("{key} is missing")),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(format!("{key} must be true or false")),
    }
}

/// A required RFC 3339 UTC time ([`timestamp_key`]).
pub(crate) fn required_timestamp(map: &Map<String, Value>, key: &str) -> Result<String, String> {
    let text = required_str(map, key)?;

    if timestamp_key(text).is_none() {
        return Err(format!("{key} must be an RFC 3339 UTC time like 2026-10-17T10:00:00Z"));
    }

    Ok(text.to_owned())
}

/// A required `"desktop"` or `"agent"`.
pub(crate) fn required_author(map: &Map<String, Value>, key: &str) -> Result<HookAuthor, String> {
    match required_str(map, key)? {
        "desktop" => Ok(HookAuthor::Desktop),
        "agent" => Ok(HookAuthor::Agent),
        other => Err(format!("{key} must be \"desktop\" or \"agent\", not {other:?}")),
    }
}

/// The name an author is written as.
pub(crate) fn author_name(author: HookAuthor) -> &'static str {
    match author {
        HookAuthor::Desktop => "desktop",
        HookAuthor::Agent => "agent",
    }
}

/// The name a scope is written as.
pub(crate) fn scope_name(scope: HookScope) -> &'static str {
    match scope {
        HookScope::Global => "global",
        HookScope::Project => "project",
    }
}

pub(crate) fn required_str<'a>(map: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    match map.get(key) {
        None => Err(format!("{key} is missing")),
        Some(Value::String(text)) => Ok(text),
        Some(_) => Err(format!("{key} must be a string")),
    }
}

/// A string, or `None` when the key is absent or `null`.
pub(crate) fn optional_str<'a>(map: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, String> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(format!("{key} must be a string or null")),
    }
}

/// A sortable form of `YYYY-MM-DDTHH:MM:SS[.fraction]Z` (year, month, day, hour, minute, second,
/// nanoseconds); `None` for anything else.
pub(crate) fn timestamp_key(text: &str) -> Option<[u32; 7]> {
    let bytes = text.as_bytes();
    let rest = text.strip_suffix('Z')?;

    if bytes.len() < 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }

    if bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }

    let number = |range: std::ops::Range<usize>| -> Option<u32> {
        let part = text.get(range)?;
        part.bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| part.parse().ok())?
    };

    let year = number(0..4)?;
    let month = number(5..7)?;
    let day = number(8..10)?;
    let hour = number(11..13)?;
    let minute = number(14..16)?;
    let second = number(17..19)?;

    let nanos = match rest.get(19..)? {
        "" => 0,
        fraction => {
            let digits = fraction.strip_prefix('.')?;
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }

            let padded: String = digits.chars().chain(std::iter::repeat('0')).take(9).collect();
            padded.parse().ok()?
        }
    };

    let valid = (1..=12).contains(&month) && (1..=31).contains(&day) && hour < 24 && minute < 60 && second < 61;

    valid.then_some([year, month, day, hour, minute, second, nanos])
}
