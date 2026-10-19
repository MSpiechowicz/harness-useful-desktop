//! An approval mode as configuration of the chat's own `opencode serve`
//! (`OPENCODE_CONFIG_CONTENT`). Pure: no I/O.
//!
//! OpenCode reads `OPENCODE_CONFIG_CONTENT` after the user's global and project configuration
//! (`config/config.ts`, merged with remeda's `mergeDeep`) and appends each agent's
//! `agent.<name>.permission` after the agent's built-in and global rules (`agent/agent.ts`:
//! `Permission.merge(item.permission, Permission.fromConfig(value.permission))`). The chat starts
//! its server once without the mode, reads every agent's rules (`GET /agent`), and starts it
//! again with each agent's mode rules ([`mode_rules`]) as `agent.<name>.permission`
//! ([`mode_config`]). Nothing is written to OpenCode's files or database.
//!
//! Two things make that exact:
//!
//! - **Keys a configuration does not use.** `mergeDeep` keeps the user's keys first, in their
//!   order, with our values, so a shared key would move our rules ahead of the user's later ones.
//!   Every key the chat writes ends in ` *`, which OpenCode's wildcard reads as "this, or this
//!   followed by a space and more" (`ls *` matches `ls`): for a permission name, which has no
//!   spaces, `read *` matches exactly `read`. The user's agent permissions therefore stay as they
//!   are with ours after them, and OpenCode's checks that compare permission names as text (a
//!   subagent session's inherited rules, the tool-output folder) see what they see without a
//!   mode. A configuration can still hold such a key: the check below then refuses the mode.
//! - **One key per permission.** An object holds a key once, a rule list may name a permission
//!   several times. A key with a `*` repeats as an equivalent variant (`* *`, `** *`, …) in list
//!   order. Each plain permission name gets one entry after those, holding in order every rule of
//!   the list that applies to it (wildcard ones too), a pattern only at its last place. For every
//!   permission the last matching rule is then the one of the list.
//!
//! [`check_applied`] checks that every agent the server reports with the mode has exactly the
//! rules it had without it, then the mode's: anything else refuses the mode instead of weakening
//! it. That covers a configuration layer OpenCode reads after ours (managed settings, a legacy
//! `mode` entry), a user key like ours, an agent permission given as a single string (`mergeDeep`
//! replaces it with our object, so the agent's rules change), and a server whose agents differ
//! from the probe's. The one rule OpenCode appends after every agent's configuration, an `allow`
//! for its own tool-output folder under `external_directory`, keeps applying.
//!
//! Also refused, before anything starts: an agent list that is empty or not a list of named
//! agents, an agent name listed twice (a configuration that renames an agent), a pattern that is
//! a number (JavaScript puts number-like keys first, which would reorder the rules), a pattern
//! with a space in a permission key, a `?` pattern that repeats, and two keys that come out the
//! same.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{Map, Value, json};

use super::modes::{Rule, matches, mode_rules};
use crate::model::ApprovalMode;
use crate::pi_family::jsonc;

/// Why an agent's rules are not the probe's followed by the mode's.
const AFTER_OURS: &str = "a managed setting, a legacy `mode` entry, a key like the app's or a permission given as one \
                          string changes";

/// The suffix of every permission key the chat writes (see the module docs).
const SUFFIX: &str = " *";

/// The configuration that gives every agent of the `GET /agent` list `agents` the rules of `mode`;
/// an error names what cannot be expressed exactly.
pub fn mode_config(mode: ApprovalMode, agents: &Value) -> Result<Value, String> {
    let mut configured = Map::new();

    for (name, rules) in listed_agents(agents)? {
        let list = mode_rules(Some(mode), &rules).unwrap_or_default();
        configured.insert(name.to_owned(), json!({ "permission": permission_object(&list)? }));
    }

    Ok(json!({ "agent": configured }))
}

/// True when a session's stored permission rules (`GET /session/{id}` `permission`) hold anything
/// but denies: OpenCode applies them after the agent's rules, so they would override the mode.
pub fn stored_overrides(permission: &Value) -> bool {
    match permission {
        Value::Null => false,
        Value::Array(rules) => rules
            .iter()
            .any(|rule| rule.get("action").and_then(Value::as_str) != Some("deny")),
        _ => true,
    }
}

/// The agents of a `GET /agent` list with their rules; an error unless it is a non-empty list of
/// agents with a name each, listed once, and readable rules.
fn listed_agents(agents: &Value) -> Result<Vec<(&str, Vec<Rule>)>, String> {
    let listed = agents
        .as_array()
        .filter(|listed| !listed.is_empty())
        .ok_or_else(|| "OpenCode listed no agents".to_owned())?;
    let mut read: Vec<(&str, Vec<Rule>)> = Vec::new();

    for agent in listed {
        let name = agent
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "OpenCode listed an agent without a name".to_owned())?;

        if read.iter().any(|(seen, _)| *seen == name) {
            return Err(format!(
                "two agents are named {name} (a configuration renames an agent)"
            ));
        }

        let rules = agent
            .get("permission")
            .cloned()
            .and_then(|rules| serde_json::from_value(rules).ok())
            .ok_or_else(|| format!("the {name} agent's permissions cannot be read"))?;
        read.push((name, rules));
    }

    Ok(read)
}

/// A permission object whose rules decide every call as `rules` do (see the module docs).
pub fn permission_object(rules: &[Rule]) -> Result<Map<String, Value>, String> {
    let mut object = Map::new();
    let mut repeats: HashMap<&str, usize> = HashMap::new();

    for rule in rules.iter().filter(|rule| is_wildcard(&rule.permission)) {
        let seen = repeats.entry(rule.permission.as_str()).or_default();
        let key = wildcard_key(&rule.permission, *seen)?;
        *seen += 1;

        let entry = json!({ checked_pattern(&rule.pattern)?: rule.action.clone() });
        insert_new(&mut object, key, entry)?;
    }

    let mut names: Vec<&str> = Vec::new();
    for rule in rules.iter().filter(|rule| !is_wildcard(&rule.permission)) {
        if !names.contains(&rule.permission.as_str()) {
            names.push(&rule.permission);
        }
    }

    for name in names {
        if name.contains(' ') {
            return Err(format!("the permission name \"{name}\" has a space"));
        }

        let mut patterns = Map::new();

        for rule in rules.iter().filter(|rule| matches(name, &rule.permission)) {
            patterns.shift_remove(&rule.pattern);
            patterns.insert(checked_pattern(&rule.pattern)?, Value::String(rule.action.clone()));
        }

        insert_new(&mut object, format!("{name}{SUFFIX}"), Value::Object(patterns))?;
    }

    Ok(object)
}

/// Adds `key` to `object`; an error when it is there already (two keys came out the same).
fn insert_new(object: &mut Map<String, Value>, key: String, value: Value) -> Result<(), String> {
    if object.contains_key(&key) {
        return Err(format!("two permission keys come out as \"{key}\""));
    }

    object.insert(key, value);
    Ok(())
}

/// `pattern` as a key of a pattern object; an error for a number-like one, which JavaScript would
/// move to the front of the object.
fn checked_pattern(pattern: &str) -> Result<String, String> {
    let number_like = !pattern.is_empty()
        && pattern.bytes().all(|byte| byte.is_ascii_digit())
        && (pattern == "0" || !pattern.starts_with('0'))
        && pattern.parse::<u64>().is_ok_and(|number| number < u64::from(u32::MAX));

    if number_like {
        return Err(format!("the pattern \"{pattern}\" is a number"));
    }

    Ok(pattern.to_owned())
}

fn is_wildcard(permission: &str) -> bool {
    permission.contains(['*', '?'])
}

/// Variant `seen` of wildcard key `permission`: one more `*` at its first `*` per repeat, then
/// [`SUFFIX`].
fn wildcard_key(permission: &str, seen: usize) -> Result<String, String> {
    if permission.contains(' ') {
        return Err(format!("the permission pattern \"{permission}\" has a space"));
    }

    if seen == 0 {
        return Ok(format!("{permission}{SUFFIX}"));
    }

    let Some(star) = permission.find('*') else {
        return Err(format!("the permission pattern \"{permission}\" repeats without a *"));
    };

    let (head, tail) = permission.split_at(star);
    Ok(format!("{head}{}{tail}{SUFFIX}", "*".repeat(seen)))
}

/// The rules OpenCode makes of a permission configuration (`Permission.fromConfig`, a string being
/// `{"*": action}`), in order, `~` and `$HOME` patterns expanded against `home`.
pub fn config_rules(permission: &Value, home: Option<&Path>) -> Vec<Rule> {
    let entries = match permission {
        Value::String(action) => return vec![Rule::new("*", "*", action)],
        Value::Object(entries) => entries,
        _ => return Vec::new(),
    };

    let mut rules = Vec::new();

    for (key, value) in entries {
        match value {
            Value::String(action) => rules.push(Rule::new(key, "*", action)),
            Value::Object(patterns) => {
                let actions = patterns
                    .iter()
                    .filter_map(|(pattern, action)| Some((pattern, action.as_str()?)));
                rules.extend(actions.map(|(pattern, action)| Rule::new(key, &expand(pattern, home), action)));
            }
            _ => {}
        }
    }

    rules
}

/// OpenCode's `expand` of a permission pattern.
fn expand(pattern: &str, home: Option<&Path>) -> String {
    let Some(home) = home.and_then(Path::to_str) else {
        return pattern.to_owned();
    };

    if let Some(rest) = pattern.strip_prefix("~/") {
        return format!("{home}/{rest}");
    }

    if pattern == "~" {
        return home.to_owned();
    }

    match pattern.strip_prefix("$HOME") {
        Some(rest) => format!("{home}{rest}"),
        None => pattern.to_owned(),
    }
}

/// remeda's `mergeDeep(target, source)`: objects merge key by key (the target's keys keep their
/// place, new ones go last), anything else is the source's.
pub fn merge_deep(target: &Value, source: &Value) -> Value {
    let (Value::Object(target), Value::Object(source)) = (target, source) else {
        return source.clone();
    };

    let mut merged = target.clone();

    for (key, value) in source {
        let next = match merged.get(key) {
            Some(existing) => merge_deep(existing, value),
            None => value.clone(),
        };
        merged.insert(key.clone(), next);
    }

    Value::Object(merged)
}

/// The inherited `OPENCODE_CONFIG_CONTENT` (JSON with comments) as an object; an error when it is
/// not one, so the chat refuses the mode rather than drop the user's settings.
pub fn inherited_config(text: Option<&str>) -> Result<Value, String> {
    let parsed = match text {
        Some(text) => jsonc::parse(text)?,
        None => Value::Null,
    };

    match parsed {
        Value::Null => Ok(json!({})),
        Value::Object(_) => Ok(parsed),
        _ => Err("it is not a JSON object".to_owned()),
    }
}

/// Checks that every agent the server reports with the mode (`applied`) has exactly the rules it
/// had without it (`probed`), then its mode rules (`config`), and only then the tool-output rule
/// when it ended with one without the mode; an error names the first agent that does not.
pub fn check_applied(probed: &Value, applied: &Value, config: &Value) -> Result<(), String> {
    let before = listed_agents(probed)?;
    let after = listed_agents(applied)?;

    let names = |agents: &[(&str, Vec<Rule>)]| agents.iter().map(|(name, _)| name.to_string()).collect::<Vec<_>>();
    if names(&before) != names(&after) {
        return Err("the agents differ between the two starts".to_owned());
    }

    for ((name, before), (_, after)) in before.iter().zip(&after) {
        let ours = config_rules(&config["agent"][*name]["permission"], None);

        if ours.is_empty() || !appended(before, &ours, after) {
            return Err(format!("{AFTER_OURS} the {name} agent's permissions"));
        }
    }

    Ok(())
}

/// True when `after` is `before` with `ours` appended, before OpenCode's trailing tool-output rule
/// when `before` ends with one.
fn appended(before: &[Rule], ours: &[Rule], after: &[Rule]) -> bool {
    let plain: Vec<Rule> = before.iter().chain(ours).cloned().collect();

    if after == plain {
        return true;
    }

    let Some((last, head)) = before.split_last() else {
        return false;
    };

    let tool_output = last.permission == "external_directory" && last.action == "allow";
    let expected: Vec<Rule> = head.iter().chain(ours).chain([last]).cloned().collect();

    tool_output && after == expected
}
