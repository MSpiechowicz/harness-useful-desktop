//! `OPENCODE_CONFIG_CONTENT` in the fake `opencode serve` (see [`super::opencode`]): like OpenCode,
//! the agents of `GET /agent` get each `agent.<name>.permission` of the content appended to their
//! rules (`Permission.fromConfig`: a string is `*` on `*`, an object gives one rule per key, a
//! nested object one per pattern, in order), before the tool-output rule OpenCode puts last (an
//! `external_directory` `allow` ending the list). A reply spec with `"ignoreConfig": true` answers
//! as recorded, to script a server whose agents do not show the content.

use serde_json::{Value, json};

/// The parsed `OPENCODE_CONFIG_CONTENT`, if set.
pub fn from_env() -> Option<Value> {
    let text = std::env::var("OPENCODE_CONFIG_CONTENT").ok()?;

    // JSON with comments, as OpenCode reads it.
    let parsed = harness_business::pi_family::jsonc::parse(&text);
    Some(parsed.unwrap_or_else(|_| crate::fail(2, "OPENCODE_CONFIG_CONTENT is not JSON")))
}

/// `agents` (a `GET /agent` body) with `config`'s agent permissions applied.
pub fn apply(config: &Value, agents: &mut Value) {
    let Some(agents) = agents.as_array_mut() else {
        return;
    };

    for agent in agents {
        let name = agent.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let added = rules(&config["agent"][&name]["permission"]);
        let Some(list) = agent.get_mut("permission").and_then(Value::as_array_mut) else {
            continue;
        };

        let tool_output = list
            .last()
            .is_some_and(|last| last["permission"] == "external_directory" && last["action"] == "allow");
        let at = if tool_output { list.len() - 1 } else { list.len() };

        list.splice(at..at, added);
    }
}

/// `Permission.fromConfig` of one agent's permission configuration.
fn rules(permission: &Value) -> Vec<Value> {
    let rule = |permission: &str, pattern: &str, action: &Value| json!({ "permission": permission, "pattern": pattern, "action": action });

    match permission {
        Value::String(_) => vec![rule("*", "*", permission)],
        Value::Object(keys) => keys
            .iter()
            .flat_map(|(key, value)| match value {
                Value::Object(patterns) => patterns
                    .iter()
                    .map(|(pattern, action)| rule(key, pattern, action))
                    .collect(),
                action => vec![rule(key, "*", action)],
            })
            .collect(),
        _ => Vec::new(),
    }
}
