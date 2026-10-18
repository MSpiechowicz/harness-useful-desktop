//! Writing one hook file: [`hook_bytes`].

use serde_json::{Map, Value};

use super::VERSION;
use crate::model::{Hook, HookAction, HookAuthor, HookScope};

/// The bytes of `hook` as a file: `base` (the object being rewritten, empty for a new file) with
/// every known field set, unknown fields kept in place, two-space JSON and a trailing newline.
pub(crate) fn hook_bytes(hook: &Hook, mut base: Map<String, Value>) -> Vec<u8> {
    let author = |author: HookAuthor| match author {
        HookAuthor::Desktop => "desktop",
        HookAuthor::Agent => "agent",
    };
    let scope = match hook.scope {
        HookScope::Global => "global",
        HookScope::Project => "project",
    };

    base.insert("version".to_owned(), Value::from(VERSION));
    base.insert("id".to_owned(), Value::from(hook.id.as_str()));
    base.insert("name".to_owned(), Value::from(hook.name.as_str()));
    base.insert("description".to_owned(), Value::from(hook.description.clone()));
    base.insert("scope".to_owned(), Value::from(scope));
    base.insert("project".to_owned(), Value::from(hook.project.clone()));
    base.insert("event".to_owned(), Value::from(hook.event.as_str()));
    base.insert("matcher".to_owned(), Value::from(hook.matcher.clone()));
    base.insert("action".to_owned(), action_value(&hook.action));
    base.insert("enabled".to_owned(), Value::from(hook.enabled));
    base.insert("createdAt".to_owned(), Value::from(hook.created_at.as_str()));
    base.insert("createdBy".to_owned(), Value::from(author(hook.created_by)));
    base.insert("updatedAt".to_owned(), Value::from(hook.updated_at.as_str()));
    base.insert("updatedBy".to_owned(), Value::from(author(hook.updated_by)));

    if hook.requires.is_empty() {
        base.remove("requires");
    } else {
        let requires = hook
            .requires
            .iter()
            .map(|requirement| Value::from(requirement.as_str()))
            .collect();
        base.insert("requires".to_owned(), Value::Array(requires));
    }

    match hook.seed {
        Some(seed) => base.insert("seed".to_owned(), Value::from(seed.as_str())),
        None => base.remove("seed"),
    };

    let mut bytes = serde_json::to_vec_pretty(&Value::Object(base)).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

fn action_value(action: &HookAction) -> Value {
    let mut map = Map::new();

    match action {
        HookAction::AddContext { text } => {
            map.insert("type".to_owned(), Value::from("addContext"));
            map.insert("text".to_owned(), Value::from(text.as_str()));
        }
        HookAction::FollowUp { text } => {
            map.insert("type".to_owned(), Value::from("followUp"));
            map.insert("text".to_owned(), Value::from(text.as_str()));
        }
        HookAction::Command { command, timeout_secs } => {
            map.insert("type".to_owned(), Value::from("command"));
            map.insert("command".to_owned(), Value::from(command.as_str()));

            if let Some(secs) = timeout_secs {
                map.insert("timeoutSecs".to_owned(), Value::from(*secs));
            }
        }
    }

    Value::Object(map)
}
