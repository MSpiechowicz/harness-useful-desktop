//! Writing one routine file: [`routine_bytes`].

use serde_json::{Map, Value};

use crate::model::{ApprovalMode, HarnessId, Routine, RoutineAction, RoutineWhen, ScriptScope};
use crate::signed_docs::fields::{VERSION, author_name, command_action_value, scope_name};

/// The bytes of `routine` as a file: `base` (the object being rewritten, empty for a new file)
/// with every known field set, unknown top-level fields kept in place, `seed` and `requires`
/// dropped, two-space JSON and a trailing newline. `when` and `action` are written whole: only
/// the fields their type has, each optional one only when set.
pub fn routine_bytes(routine: &Routine, mut base: Map<String, Value>) -> Vec<u8> {
    base.insert("version".to_owned(), Value::from(VERSION));
    base.insert("id".to_owned(), Value::from(routine.id.as_str()));
    base.insert("name".to_owned(), Value::from(routine.name.as_str()));
    base.insert("description".to_owned(), Value::from(routine.description.clone()));
    base.insert("scope".to_owned(), Value::from(scope_name(routine.scope)));
    base.insert("project".to_owned(), Value::from(routine.project.clone()));
    base.insert("when".to_owned(), when_value(&routine.when));
    base.insert("action".to_owned(), action_value(&routine.action));
    base.insert("enabled".to_owned(), Value::from(routine.enabled));
    base.insert("createdAt".to_owned(), Value::from(routine.created_at.as_str()));
    base.insert("createdBy".to_owned(), Value::from(author_name(routine.created_by)));
    base.insert("updatedAt".to_owned(), Value::from(routine.updated_at.as_str()));
    base.insert("updatedBy".to_owned(), Value::from(author_name(routine.updated_by)));

    base.remove("seed");
    base.remove("requires");

    let mut bytes = serde_json::to_vec_pretty(&Value::Object(base)).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

fn when_value(when: &RoutineWhen) -> Value {
    let mut map = Map::new();
    map.insert("type".to_owned(), Value::from(when.kind().as_str()));

    match when {
        RoutineWhen::Daily { at } | RoutineWhen::Weekdays { at } => {
            map.insert("at".to_owned(), Value::from(at.as_str()));
        }
        RoutineWhen::EveryMinutes { every } | RoutineWhen::EveryHours { every } => {
            map.insert("every".to_owned(), Value::from(*every));
        }
        RoutineWhen::Cron { expression } => {
            map.insert("expression".to_owned(), Value::from(expression.as_str()));
        }
        RoutineWhen::AppStart => {}
    }

    Value::Object(map)
}

fn action_value(action: &RoutineAction) -> Value {
    let mut map = Map::new();

    match action {
        RoutineAction::Agent {
            harness,
            model,
            prompt,
            approval_mode,
            trust_once,
            max_minutes,
        } => {
            map.insert("type".to_owned(), Value::from("agent"));
            map.insert("harness".to_owned(), Value::from(harness_name(*harness)));

            let model = model.as_ref().map(|model| {
                let mut pair = Map::new();
                pair.insert("provider".to_owned(), Value::from(model.provider.as_str()));
                pair.insert("id".to_owned(), Value::from(model.id.as_str()));

                if let Some(thinking) = &model.thinking {
                    pair.insert("thinking".to_owned(), Value::from(thinking.as_str()));
                }

                Value::Object(pair)
            });
            map.insert("model".to_owned(), model.unwrap_or(Value::Null));
            map.insert("prompt".to_owned(), Value::from(prompt.as_str()));

            if let Some(mode) = approval_mode {
                map.insert("approvalMode".to_owned(), Value::from(approval_mode_name(*mode)));
            }

            if *trust_once {
                map.insert("trustOnce".to_owned(), Value::from(true));
            }

            if let Some(minutes) = max_minutes {
                map.insert("maxMinutes".to_owned(), Value::from(*minutes));
            }
        }
        RoutineAction::Command { command, timeout_secs } => return command_action_value(command, *timeout_secs),
        RoutineAction::Script { name, scope } => {
            map.insert("type".to_owned(), Value::from("script"));
            map.insert("name".to_owned(), Value::from(name.as_str()));

            let scope = match scope {
                ScriptScope::Repo => "repo",
                ScriptScope::Global => "global",
            };
            map.insert("scope".to_owned(), Value::from(scope));
        }
    }

    Value::Object(map)
}

fn harness_name(harness: HarnessId) -> &'static str {
    harness.binary_name()
}

fn approval_mode_name(mode: ApprovalMode) -> &'static str {
    match mode {
        ApprovalMode::AlwaysAsk => "alwaysAsk",
        ApprovalMode::Write => "write",
        ApprovalMode::Yolo => "yolo",
        ApprovalMode::Auto => "auto",
    }
}
