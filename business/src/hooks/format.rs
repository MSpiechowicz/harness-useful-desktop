//! Reading and writing one hook file (version 1, see `docs/hooks-format.md`).
//!
//! [`parse_hook_file`] checks everything the format doc lists and names the first problem found.
//! [`hook_bytes`] writes a hook the way both apps do: the known fields in the doc's order (or in
//! their place in the file being rewritten), unknown top-level fields kept, two-space JSON with a
//! trailing newline.

use std::path::Path;

use serde_json::{Map, Value};

use super::unicode::is_hidden_char;
use crate::model::{Hook, HookAction, HookAuthor, HookEvent, HookRequirement, HookScope, HookSeed};

mod write;

pub(crate) use write::hook_bytes;

/// A larger file is invalid and is not parsed.
pub const MAX_HOOK_FILE_BYTES: usize = 16 * 1024;

/// Longest `name`, in characters.
pub const MAX_NAME_CHARS: usize = 80;

/// Longest `description`, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 500;

/// Longest `addContext` or `followUp` text, in characters.
pub const MAX_TEXT_CHARS: usize = 4096;

/// Longest `command`, in characters.
pub const MAX_COMMAND_CHARS: usize = 2048;

/// Longest `matcher`, in characters.
pub const MAX_MATCHER_CHARS: usize = 200;

/// `timeoutSecs` when the file has none.
pub const DEFAULT_TIMEOUT_SECS: u32 = 10;

/// Largest `timeoutSecs`.
pub const MAX_TIMEOUT_SECS: u32 = 30;

/// The only file version.
const VERSION: u64 = 1;

/// True for a hook id: 32 lowercase hex characters.
pub fn is_hook_id(text: &str) -> bool {
    text.len() == 32
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Longest repository key of a team hook's `project`, in bytes.
pub const MAX_REPO_KEY_BYTES: usize = 512;

/// What a project hook's `project` holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectForm {
    /// A hook file: the absolute project root.
    Root,
    /// A team hook's body: the repository key (`git:host/owner/repo`).
    RepoKey,
}

/// The hook in `bytes`, the content of `<stem>.json`, or why the file is invalid.
pub fn parse_hook_file(stem: &str, bytes: &[u8]) -> Result<Hook, String> {
    parse_bytes(stem, bytes, ProjectForm::Root)
}

/// The hook in a team hook's body (see "Team hooks" in the format doc): a hook file whose
/// `project` is a `git:` repository key instead of a project root. Other team rules (no `seed`)
/// are the caller's.
pub(crate) fn parse_team_file(stem: &str, bytes: &[u8]) -> Result<Hook, String> {
    parse_bytes(stem, bytes, ProjectForm::RepoKey)
}

fn parse_bytes(stem: &str, bytes: &[u8], form: ProjectForm) -> Result<Hook, String> {
    if bytes.len() > MAX_HOOK_FILE_BYTES {
        return Err("the file is larger than 16 KiB".to_owned());
    }

    let text = std::str::from_utf8(bytes).map_err(|_| "the file is not UTF-8".to_owned())?;
    let value: Value = serde_json::from_str(text).map_err(|_| "the file is not JSON".to_owned())?;

    let Value::Object(map) = value else {
        return Err("the file is not a JSON object".to_owned());
    };

    parse_map(stem, &map, form)
}

/// The `name` of an invalid file, when it is a string (for the list).
pub(crate) fn loose_name(bytes: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let name = value.get("name")?.as_str()?;

    Some(name.chars().take(MAX_NAME_CHARS).collect())
}

fn parse_map(stem: &str, map: &Map<String, Value>, form: ProjectForm) -> Result<Hook, String> {
    match map.get("version") {
        None => return Err("version is missing".to_owned()),
        Some(version) if version.as_u64() != Some(VERSION) => return Err("version must be 1".to_owned()),
        Some(_) => {}
    }

    let id = required_str(map, "id")?;
    if !is_hook_id(id) {
        return Err("id must be 32 lowercase hex characters".to_owned());
    }

    if id != stem {
        return Err("id does not match the file name".to_owned());
    }

    let name = required_str(map, "name")?;
    check_name(name)?;

    let description = optional_str(map, "description")?;
    if let Some(description) = description {
        check_description(description)?;
    }

    let (scope, project) = parse_scope(map, form)?;

    let event = required_str(map, "event")?;
    let event = HookEvent::parse(event).ok_or_else(|| format!("unknown event {event:?}"))?;

    let matcher = optional_str(map, "matcher")?;
    if let Some(matcher) = matcher {
        check_matcher(event, matcher)?;
    }

    let action = parse_action(map.get("action"))?;
    check_combination(event, &action)?;

    let enabled = match map.get("enabled") {
        None => return Err("enabled is missing".to_owned()),
        Some(Value::Bool(enabled)) => *enabled,
        Some(_) => return Err("enabled must be true or false".to_owned()),
    };

    let requires = parse_requires(map.get("requires"))?;

    let seed = match optional_str(map, "seed")? {
        None => None,
        Some(seed) => Some(HookSeed::parse(seed).ok_or_else(|| format!("unknown seed {seed:?}"))?),
    };

    let created_at = required_timestamp(map, "createdAt")?;
    let created_by = required_author(map, "createdBy")?;
    let updated_at = required_timestamp(map, "updatedAt")?;
    let updated_by = required_author(map, "updatedBy")?;

    Ok(Hook {
        id: id.to_owned(),
        name: name.to_owned(),
        description: description.map(str::to_owned),
        scope,
        project,
        event,
        matcher: matcher.map(str::to_owned),
        action,
        enabled,
        requires,
        seed,
        created_at,
        created_by,
        updated_at,
        updated_by,
    })
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
fn check_description(description: &str) -> Result<(), String> {
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!("description is longer than {MAX_DESCRIPTION_CHARS} characters"));
    }

    if description.chars().any(is_hidden_char) {
        return Err("description contains an invisible character".to_owned());
    }

    Ok(())
}

fn parse_scope(map: &Map<String, Value>, form: ProjectForm) -> Result<(HookScope, Option<String>), String> {
    let scope = required_str(map, "scope")?;
    let project = optional_str(map, "project")?;

    match (scope, project) {
        ("global", None) => Ok((HookScope::Global, None)),
        ("global", Some(_)) => Err("a global hook has no project".to_owned()),
        ("project", None) => Err("a project hook needs a project".to_owned()),
        ("project", Some(project)) => {
            check_project(project, form)?;
            Ok((HookScope::Project, Some(project.to_owned())))
        }
        (other, _) => Err(format!("unknown scope {other:?}")),
    }
}

/// A project root is absolute; a repository key starts with `git:`, is at most
/// [`MAX_REPO_KEY_BYTES`] long and has no spaces, control or invisible characters.
fn check_project(project: &str, form: ProjectForm) -> Result<(), String> {
    match form {
        ProjectForm::Root if !Path::new(project).has_root() => Err("project must be an absolute path".to_owned()),
        ProjectForm::Root => Ok(()),
        ProjectForm::RepoKey => {
            let plain = project.len() <= MAX_REPO_KEY_BYTES
                && project.len() > REPO_PREFIX.len()
                && project.starts_with(REPO_PREFIX)
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

/// What a shared project hook's repository key starts with.
const REPO_PREFIX: &str = "git:";

/// `name *( "|" name )`, `name = 1*[A-Za-z0-9_.:-]`, at most [`MAX_MATCHER_CHARS`], `toolEnd` only.
pub(crate) fn check_matcher(event: HookEvent, matcher: &str) -> Result<(), String> {
    if event != HookEvent::ToolEnd {
        return Err("only a toolEnd hook has a matcher".to_owned());
    }

    if matcher.chars().count() > MAX_MATCHER_CHARS {
        return Err(format!("matcher is longer than {MAX_MATCHER_CHARS} characters"));
    }

    let allowed = |character: char| character.is_ascii_alphanumeric() || "_.:-".contains(character);
    let valid = matcher
        .split('|')
        .all(|name| !name.is_empty() && name.chars().all(allowed));

    if !valid {
        return Err("matcher must be tool names joined by |".to_owned());
    }

    Ok(())
}

fn parse_action(action: Option<&Value>) -> Result<HookAction, String> {
    let Some(action) = action else {
        return Err("action is missing".to_owned());
    };

    let Value::Object(action) = action else {
        return Err("action must be an object".to_owned());
    };

    let kind = action
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "action.type is missing".to_owned())?;

    let text = |field: &str| -> Result<String, String> {
        let text = action
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("action.{field} is missing"))?;

        check_length(&format!("action.{field}"), text, MAX_TEXT_CHARS)?;
        Ok(text.to_owned())
    };

    match kind {
        "addContext" => Ok(HookAction::AddContext { text: text("text")? }),
        "followUp" => Ok(HookAction::FollowUp { text: text("text")? }),
        "command" => {
            let command = action
                .get("command")
                .and_then(Value::as_str)
                .ok_or_else(|| "action.command is missing".to_owned())?;
            check_length("action.command", command, MAX_COMMAND_CHARS)?;

            let timeout_secs = match action.get("timeoutSecs") {
                None | Some(Value::Null) => None,
                Some(value) => Some(parse_timeout(value)?),
            };

            Ok(HookAction::Command {
                command: command.to_owned(),
                timeout_secs,
            })
        }
        other => Err(format!("unknown action type {other:?}")),
    }
}

fn parse_timeout(value: &Value) -> Result<u32, String> {
    value
        .as_u64()
        .filter(|secs| (1..=u64::from(MAX_TIMEOUT_SECS)).contains(secs))
        .and_then(|secs| u32::try_from(secs).ok())
        .ok_or_else(|| format!("action.timeoutSecs must be a whole number from 1 to {MAX_TIMEOUT_SECS}"))
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

/// Which actions an event takes (the table in the format doc).
pub(crate) fn check_combination(event: HookEvent, action: &HookAction) -> Result<(), String> {
    let allowed = match action {
        HookAction::AddContext { .. } => matches!(event, HookEvent::SessionStart | HookEvent::PromptSubmit),
        HookAction::FollowUp { .. } => matches!(event, HookEvent::ToolEnd | HookEvent::TurnEnd),
        HookAction::Command { .. } => true,
    };

    if !allowed {
        return Err(format!("a {} hook cannot use this action", event.as_str()));
    }

    Ok(())
}

fn parse_requires(value: Option<&Value>) -> Result<Vec<HookRequirement>, String> {
    let items = match value {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(items)) => items,
        Some(_) => return Err("requires must be a list".to_owned()),
    };

    let mut requires = Vec::new();

    for item in items {
        let requirement = match item.as_str() {
            Some(name) => HookRequirement::parse(name).ok_or_else(|| format!("unknown requirement {name:?}"))?,
            None => return Err("requires must list strings".to_owned()),
        };

        if requires.contains(&requirement) {
            return Err("requires lists a requirement twice".to_owned());
        }

        requires.push(requirement);
    }

    Ok(requires)
}

fn required_timestamp(map: &Map<String, Value>, key: &str) -> Result<String, String> {
    let text = required_str(map, key)?;

    if timestamp_key(text).is_none() {
        return Err(format!("{key} must be an RFC 3339 UTC time like 2026-10-17T10:00:00Z"));
    }

    Ok(text.to_owned())
}

fn required_author(map: &Map<String, Value>, key: &str) -> Result<HookAuthor, String> {
    match required_str(map, key)? {
        "desktop" => Ok(HookAuthor::Desktop),
        "agent" => Ok(HookAuthor::Agent),
        other => Err(format!("{key} must be \"desktop\" or \"agent\", not {other:?}")),
    }
}

fn required_str<'a>(map: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    match map.get(key) {
        None => Err(format!("{key} is missing")),
        Some(Value::String(text)) => Ok(text),
        Some(_) => Err(format!("{key} must be a string")),
    }
}

/// A string, or `None` when the key is absent or `null`.
fn optional_str<'a>(map: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, String> {
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
