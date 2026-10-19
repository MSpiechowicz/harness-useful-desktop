//! Reading and writing one hook file (version 1, see `docs/hooks-format.md`).
//!
//! [`parse_hook_file`] checks everything the format doc lists and names the first problem found;
//! the fields every signed document shares are checked by [`crate::signed_docs::fields`].
//! [`hook_bytes`] writes a hook the way both apps do: the known fields in the doc's order (or in
//! their place in the file being rewritten), unknown top-level fields kept, two-space JSON with a
//! trailing newline.

use serde_json::{Map, Value};

use super::HookKind;
use crate::model::{Hook, HookAction, HookEvent, HookRequirement, HookSeed};
use crate::signed_docs::fields::{
    ProjectForm, file_object, optional_str, parse_head, parse_stamps, required_bool, required_str, typed_object,
};

mod write;

pub use crate::signed_docs::fields::{MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS, MAX_REPO_KEY_BYTES};
pub(crate) use crate::signed_docs::fields::{check_length, loose_name};
pub(crate) use write::hook_bytes;

/// A larger file is invalid and is not parsed.
pub const MAX_HOOK_FILE_BYTES: usize = 16 * 1024;

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

/// True for a hook id: 32 lowercase hex characters.
pub fn is_hook_id(text: &str) -> bool {
    crate::signed_docs::is_doc_id(text)
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
    let map = file_object::<HookKind>(bytes)?;

    parse_map(stem, &map, form)
}

fn parse_map(stem: &str, map: &Map<String, Value>, form: ProjectForm) -> Result<Hook, String> {
    let head = parse_head::<HookKind>(map, stem, form)?;

    let event = required_str(map, "event")?;
    let event = HookEvent::parse(event).ok_or_else(|| format!("unknown event {event:?}"))?;

    let matcher = optional_str(map, "matcher")?;
    if let Some(matcher) = matcher {
        check_matcher(event, matcher)?;
    }

    let action = parse_action(map.get("action"))?;
    check_combination(event, &action)?;

    let enabled = required_bool(map, "enabled")?;

    let requires = parse_requires(map.get("requires"))?;

    let seed = match optional_str(map, "seed")? {
        None => None,
        Some(seed) => Some(HookSeed::parse(seed).ok_or_else(|| format!("unknown seed {seed:?}"))?),
    };

    let stamps = parse_stamps(map)?;

    Ok(Hook {
        id: head.id,
        name: head.name,
        description: head.description,
        scope: head.scope,
        project: head.project,
        event,
        matcher: matcher.map(str::to_owned),
        action,
        enabled,
        requires,
        seed,
        created_at: stamps.created_at,
        created_by: stamps.created_by,
        updated_at: stamps.updated_at,
        updated_by: stamps.updated_by,
    })
}

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
    let (action, kind) = typed_object(action, "action")?;

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
