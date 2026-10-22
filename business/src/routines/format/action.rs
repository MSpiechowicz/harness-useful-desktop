//! The `action` object of a routine file: read into a [`RoutineAction`] ([`parse_action`]), then
//! checked ([`check_action`]) against the format doc's limits and harness rules.

use serde_json::{Map, Value};

use super::{check_plain_text, harness_names, has_chat, takes_approval_mode, takes_auto, takes_trust_once, takes_yolo};
use crate::model::{ApprovalMode, HarnessId, HookScope, RoutineAction, RoutineModel, ScriptScope};
use crate::signed_docs::fields::typed_object;
use crate::signed_docs::unicode::is_hidden_char;

/// Longest agent prompt, in characters.
pub const MAX_PROMPT_CHARS: usize = 8192;

/// Longest command, in characters.
pub const MAX_COMMAND_CHARS: usize = 2048;

/// Longest model provider or id, in characters.
pub const MAX_MODEL_CHARS: usize = 200;

/// `maxMinutes` when the file has none.
pub const DEFAULT_MAX_MINUTES: u32 = 30;

/// Largest `maxMinutes`.
pub const MAX_MAX_MINUTES: u32 = 240;

/// `timeoutSecs` when the file has none.
pub const DEFAULT_TIMEOUT_SECS: u32 = 300;

/// Largest `timeoutSecs`.
pub const MAX_TIMEOUT_SECS: u32 = 3600;

/// Longest script name, in characters.
const MAX_SCRIPT_NAME_CHARS: usize = 64;

/// The `action` object as its type, its values not yet checked ([`check_action`]). Unknown fields
/// are ignored; `null` means the default of an optional field.
pub(super) fn parse_action(action: Option<&Value>) -> Result<RoutineAction, String> {
    let (action, kind) = typed_object(action, "action")?;

    match kind {
        "agent" => parse_agent(action),
        "command" => Ok(RoutineAction::Command {
            command: text(action, "command")?,
            timeout_secs: whole(action, "timeoutSecs")?,
        }),
        "script" => {
            let scope = match text(action, "scope")?.as_str() {
                "repo" => ScriptScope::Repo,
                "global" => ScriptScope::Global,
                other => return Err(format!("action.scope must be \"repo\" or \"global\", not {other:?}")),
            };

            Ok(RoutineAction::Script {
                name: text(action, "name")?,
                scope,
            })
        }
        other => Err(format!("unknown action type {other:?}")),
    }
}

fn parse_agent(action: &Map<String, Value>) -> Result<RoutineAction, String> {
    let name = text(action, "harness")?;
    let Some(harness) = HarnessId::ALL.into_iter().find(|id| id.binary_name() == name) else {
        return Err(format!("unknown harness {name:?}"));
    };

    let model = match action.get("model") {
        None | Some(Value::Null) => None,
        Some(Value::Object(model)) => Some(RoutineModel {
            provider: model_text(model, "provider")?,
            id: model_text(model, "id")?,
        }),
        Some(_) => return Err("action.model must be an object with provider and id, or null".to_owned()),
    };

    let approval_mode = match action.get("approvalMode") {
        None | Some(Value::Null) => None,
        Some(Value::String(mode)) => Some(match mode.as_str() {
            "alwaysAsk" => ApprovalMode::AlwaysAsk,
            "write" => ApprovalMode::Write,
            "yolo" => ApprovalMode::Yolo,
            "auto" => ApprovalMode::Auto,
            other => return Err(format!("unknown action.approvalMode {other:?}")),
        }),
        Some(_) => return Err("action.approvalMode must be a string or null".to_owned()),
    };

    let trust_once = match action.get("trustOnce") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(trust)) => *trust,
        Some(_) => return Err("action.trustOnce must be true or false".to_owned()),
    };

    Ok(RoutineAction::Agent {
        harness,
        model,
        prompt: text(action, "prompt")?,
        approval_mode,
        trust_once,
        max_minutes: whole(action, "maxMinutes")?,
    })
}

/// A required string field of the action.
fn text(action: &Map<String, Value>, field: &str) -> Result<String, String> {
    match action.get(field) {
        None => Err(format!("action.{field} is missing")),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(_) => Err(format!("action.{field} must be a string")),
    }
}

/// A required string field of the model object.
fn model_text(model: &Map<String, Value>, field: &str) -> Result<String, String> {
    match model.get(field) {
        Some(Value::String(text)) => Ok(text.clone()),
        _ => Err(format!("action.model.{field} must be a string")),
    }
}

/// An optional whole number (`null` or absent: `None`); a fraction, a negative number or another
/// type is refused.
fn whole(action: &Map<String, Value>, field: &str) -> Result<Option<u32>, String> {
    match action.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .map(Some)
            .ok_or_else(|| format!("action.{field} must be a whole number")),
    }
}

/// The values of `action` in a routine of `scope`: an agent harness with a chat, a prompt, model,
/// approval mode and trust that fit the harness, limits in range, a valid script name, and a
/// repository script only in a project routine.
pub fn check_action(action: &RoutineAction, scope: HookScope) -> Result<(), String> {
    match action {
        RoutineAction::Agent {
            harness,
            model,
            prompt,
            approval_mode,
            trust_once,
            max_minutes,
        } => {
            if !has_chat(*harness) {
                return Err(format!("an agent routine runs {}", harness_names(has_chat)));
            }

            check_plain_text("action.prompt", prompt, MAX_PROMPT_CHARS)?;

            if let Some(model) = model {
                check_model_part("provider", &model.provider)?;
                check_model_part("id", &model.id)?;
            }

            if approval_mode.is_some() && !takes_approval_mode(*harness) {
                return Err(format!(
                    "action.approvalMode is for {} only",
                    harness_names(takes_approval_mode)
                ));
            }

            if *approval_mode == Some(ApprovalMode::Yolo) && !takes_yolo(*harness) {
                return Err(format!(
                    "action.approvalMode \"yolo\" is for {} only",
                    harness_names(takes_yolo)
                ));
            }

            if *approval_mode == Some(ApprovalMode::Auto) && !takes_auto(*harness) {
                return Err(format!(
                    "action.approvalMode \"auto\" is for {} only",
                    harness_names(takes_auto)
                ));
            }

            if *trust_once && !takes_trust_once(*harness) {
                return Err(format!(
                    "action.trustOnce is for {} only",
                    harness_names(takes_trust_once)
                ));
            }

            check_range("maxMinutes", *max_minutes, MAX_MAX_MINUTES)
        }
        RoutineAction::Command { command, timeout_secs } => {
            check_plain_text("action.command", command, MAX_COMMAND_CHARS)?;
            check_range("timeoutSecs", *timeout_secs, MAX_TIMEOUT_SECS)
        }
        RoutineAction::Script { name, scope: place } => {
            if !is_script_name(name) {
                return Err("action.name must be a script name: a-z, 0-9, _ and -, at most 64".to_owned());
            }

            if *place == ScriptScope::Repo && scope == HookScope::Global {
                return Err("a global routine has no repository: use a global script".to_owned());
            }

            Ok(())
        }
    }
}

fn check_model_part(field: &str, text: &str) -> Result<(), String> {
    let chars = text.chars().count();

    if chars == 0 || chars > MAX_MODEL_CHARS {
        return Err(format!(
            "action.model.{field} must be 1 to {MAX_MODEL_CHARS} characters"
        ));
    }

    if text
        .chars()
        .any(|character| character.is_control() || is_hidden_char(character))
    {
        return Err(format!(
            "action.model.{field} contains a control or invisible character"
        ));
    }

    Ok(())
}

fn check_range(field: &str, value: Option<u32>, max: u32) -> Result<(), String> {
    match value {
        Some(value) if !(1..=max).contains(&value) => {
            Err(format!("action.{field} must be a whole number from 1 to {max}"))
        }
        _ => Ok(()),
    }
}

/// Harness Memory's script name rule: `^[a-z0-9][a-z0-9_-]{0,63}$`.
pub fn is_script_name(name: &str) -> bool {
    let mut chars = name.chars();

    let first_ok = chars
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit());
    let rest_ok = chars.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_' || character == '-'
    });

    first_ok && rest_ok && name.len() <= MAX_SCRIPT_NAME_CHARS
}
