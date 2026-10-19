//! Reading and writing one routine file (version 1, see `docs/routines-format.md`).
//!
//! [`parse_routine_file`] checks everything the format doc lists and names the first problem
//! found; the fields every signed document shares are checked by
//! [`crate::signed_docs::fields`]. `when` and `action` are read into their types first and then
//! checked by [`check_when`] and [`check_action`], the same checks a draft and a schedule preview
//! get. [`routine_bytes`] writes a routine the way both apps do: the known fields in the doc's
//! order (or in their place in the file being rewritten), unknown top-level fields kept,
//! two-space JSON with a trailing newline.

use serde_json::{Map, Value};

use super::RoutineKind;
use super::cron::CronExpr;
use crate::harness::HarnessRegistry;
use crate::model::{Capabilities, HarnessId, Routine, RoutineWhen};
use crate::signed_docs::fields::{
    ProjectForm, check_length, file_object, parse_head, parse_stamps, required_bool, typed_object,
};
use crate::signed_docs::unicode::is_hidden_char;

mod action;
mod write;

pub use crate::signed_docs::fields::{MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS, MAX_REPO_KEY_BYTES};
pub use action::{
    DEFAULT_MAX_MINUTES, DEFAULT_TIMEOUT_SECS, MAX_COMMAND_CHARS, MAX_MAX_MINUTES, MAX_MODEL_CHARS, MAX_PROMPT_CHARS,
    MAX_TIMEOUT_SECS, check_action, is_script_name,
};
pub use write::routine_bytes;

/// A larger file is invalid and is not parsed.
pub const MAX_ROUTINE_FILE_BYTES: usize = 16 * 1024;

/// Longest cron expression, in characters.
pub const MAX_CRON_CHARS: usize = 100;

/// Largest `every` of an `everyMinutes` schedule.
pub const MAX_EVERY_MINUTES: u32 = 720;

/// Largest `every` of an `everyHours` schedule.
pub const MAX_EVERY_HOURS: u32 = 24;

/// The routine in `bytes`, the content of `<stem>.json`, or why the file is invalid.
pub fn parse_routine_file(stem: &str, bytes: &[u8]) -> Result<Routine, String> {
    parse_bytes(stem, bytes, ProjectForm::Root)
}

/// The routine in a team routine's body (see "Team routines" in the format doc): a routine file
/// whose `project` is a `git:` repository key instead of a project root.
pub fn parse_team_routine(stem: &str, bytes: &[u8]) -> Result<Routine, String> {
    parse_bytes(stem, bytes, ProjectForm::RepoKey)
}

fn parse_bytes(stem: &str, bytes: &[u8], form: ProjectForm) -> Result<Routine, String> {
    let map = file_object::<RoutineKind>(bytes)?;

    parse_map(stem, &map, form)
}

fn parse_map(stem: &str, map: &Map<String, Value>, form: ProjectForm) -> Result<Routine, String> {
    let head = parse_head::<RoutineKind>(map, stem, form)?;

    let when = parse_when(map.get("when"))?;
    check_when(&when)?;

    let action = action::parse_action(map.get("action"))?;
    check_action(&action, head.scope)?;

    let enabled = required_bool(map, "enabled")?;

    for reserved in ["seed", "requires"] {
        if map.get(reserved).is_some_and(|value| !value.is_null()) {
            return Err(format!("a routine has no {reserved}"));
        }
    }

    let stamps = parse_stamps(map)?;

    Ok(Routine {
        id: head.id,
        name: head.name,
        description: head.description,
        scope: head.scope,
        project: head.project,
        when,
        action,
        enabled,
        created_at: stamps.created_at,
        created_by: stamps.created_by,
        updated_at: stamps.updated_at,
        updated_by: stamps.updated_by,
    })
}

/// The `when` object as its type, its values not yet checked ([`check_when`]). Unknown fields are
/// ignored.
fn parse_when(when: Option<&Value>) -> Result<RoutineWhen, String> {
    let (when, kind) = typed_object(when, "when")?;

    let text = |field: &str| -> Result<String, String> {
        match when.get(field) {
            None => Err(format!("when.{field} is missing")),
            Some(Value::String(text)) => Ok(text.clone()),
            Some(_) => Err(format!("when.{field} must be a string")),
        }
    };

    let every = || -> Result<u32, String> {
        let value = when.get("every").ok_or_else(|| "when.every is missing".to_owned())?;

        value
            .as_u64()
            .and_then(|every| u32::try_from(every).ok())
            .ok_or_else(|| "when.every must be a whole number".to_owned())
    };

    match kind {
        "daily" => Ok(RoutineWhen::Daily { at: text("at")? }),
        "weekdays" => Ok(RoutineWhen::Weekdays { at: text("at")? }),
        "everyMinutes" => Ok(RoutineWhen::EveryMinutes { every: every()? }),
        "everyHours" => Ok(RoutineWhen::EveryHours { every: every()? }),
        "cron" => Ok(RoutineWhen::Cron {
            expression: text("expression")?,
        }),
        "appStart" => Ok(RoutineWhen::AppStart),
        other => Err(format!("unknown when.type {other:?}")),
    }
}

/// The values of `when`: an `HH:MM` time, an `every` in its range, a cron expression that parses
/// and can fire.
pub fn check_when(when: &RoutineWhen) -> Result<(), String> {
    match when {
        RoutineWhen::Daily { at } | RoutineWhen::Weekdays { at } => {
            if parse_at(at).is_none() {
                return Err("when.at must be a time like 09:00 (HH:MM, 24-hour)".to_owned());
            }
        }
        RoutineWhen::EveryMinutes { every } => check_every(*every, MAX_EVERY_MINUTES)?,
        RoutineWhen::EveryHours { every } => check_every(*every, MAX_EVERY_HOURS)?,
        RoutineWhen::Cron { expression } => {
            if expression.chars().count() > MAX_CRON_CHARS {
                return Err(format!("when.expression is longer than {MAX_CRON_CHARS} characters"));
            }

            CronExpr::parse(expression)?;
        }
        RoutineWhen::AppStart => {}
    }

    Ok(())
}

fn check_every(every: u32, max: u32) -> Result<(), String> {
    if (1..=max).contains(&every) {
        Ok(())
    } else {
        Err(format!("when.every must be a whole number from 1 to {max}"))
    }
}

/// The hour and minute of `HH:MM` (`^([01][0-9]|2[0-3]):[0-5][0-9]$`), `None` for anything else.
pub fn parse_at(at: &str) -> Option<(i8, i8)> {
    let bytes = at.as_bytes();

    if bytes.len() != 5 || bytes[2] != b':' {
        return None;
    }

    let digits = [bytes[0], bytes[1], bytes[3], bytes[4]];
    if !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }

    let [h1, h2, m1, m2] = digits.map(|digit| (digit - b'0') as i8);
    let hour = h1 * 10 + h2;
    let minute = m1 * 10 + m2;

    (hour < 24 && minute < 60).then_some((hour, minute))
}

/// 1 to `max` characters, no invisible characters and no control characters but tab, line feed
/// and carriage return: what the user reviews is what runs.
pub(crate) fn check_plain_text(field: &str, text: &str, max: usize) -> Result<(), String> {
    check_length(field, text, max)?;

    if text.chars().any(is_hidden_char) {
        return Err(format!("{field} contains an invisible character"));
    }

    let allowed = |character: char| matches!(character, '\t' | '\n' | '\r');

    if text
        .chars()
        .any(|character| character.is_control() && !allowed(character))
    {
        return Err(format!("{field} contains a control character"));
    }

    Ok(())
}

/// What `harness` can do, as this app ships it.
fn capabilities(harness: HarnessId) -> Capabilities {
    HarnessRegistry::with_defaults()
        .get(harness)
        .map(|harness| harness.capabilities())
        .unwrap_or_default()
}

/// The harnesses a routine can chat with.
pub(crate) fn has_chat(harness: HarnessId) -> bool {
    capabilities(harness).chat
}

/// The harnesses whose agent routine may set an approval mode (OMP, OpenCode).
pub(crate) fn takes_approval_mode(harness: HarnessId) -> bool {
    let capabilities = capabilities(harness);

    capabilities.chat && capabilities.approval_modes
}

/// The harnesses whose agent routine may trust the project once (PI).
pub(crate) fn takes_trust_once(harness: HarnessId) -> bool {
    let capabilities = capabilities(harness);

    capabilities.chat && capabilities.project_trust
}

/// The CLI names of the harnesses that pass `allowed`, in display order, as "omp, pi or opencode".
pub(crate) fn harness_names(allowed: fn(HarnessId) -> bool) -> String {
    let names: Vec<&str> = HarnessId::ALL
        .into_iter()
        .filter(|harness| allowed(*harness))
        .map(HarnessId::binary_name)
        .collect();

    match names.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
    }
}
