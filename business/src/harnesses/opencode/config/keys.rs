//! The plugin, MCP server and command keys of one OpenCode settings file, per generation.
//!
//! OpenCode 1 reads `plugin`, `mcp.<name>` and `command`. OpenCode 2 reads `plugins`,
//! `mcp.servers.<name>` and `commands`, and normalizes the OpenCode 1 keys of the same file into
//! them (`core/src/config/normalize.ts`, 2.1.2): the legacy entries come first, and a native entry
//! replaces a legacy one of the same name.

use serde_json::{Map, Value};

use crate::harness::OpenCodeGeneration;

/// One plugin entry of a settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PluginEntry {
    /// A spec string: an npm spec, a path or a URL.
    Spec(String),
    /// An entry with options: `[spec, options]`, or on OpenCode 2 `{package, options}` (its spec,
    /// when it is a string).
    WithOptions(Option<String>),
    /// `-<target>` (OpenCode 2): removes a plugin an earlier entry or a plugin directory added.
    Remove(String),
    /// Anything else (OpenCode 2 skips it).
    Other,
}

/// The plugin entries of `document` in file order: `plugin` on OpenCode 1, `plugin` then
/// `plugins` on OpenCode 2.
pub(crate) fn plugin_entries(document: &Value, generation: OpenCodeGeneration) -> Vec<PluginEntry> {
    let legacy = list(document, "plugin");

    if generation == OpenCodeGeneration::V1 {
        return legacy.iter().map(v1_plugin).collect();
    }

    let native = list(document, "plugins");
    legacy
        .iter()
        .map(v2_legacy_plugin)
        .chain(native.iter().map(v2_plugin))
        .collect()
}

fn list<'a>(document: &'a Value, key: &str) -> &'a [Value] {
    document.get(key).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

/// OpenCode 1: a spec, or `[spec, options]` (any list counts, its first item is the spec).
fn v1_plugin(item: &Value) -> PluginEntry {
    match item {
        Value::String(spec) => PluginEntry::Spec(spec.clone()),
        Value::Array(pair) => PluginEntry::WithOptions(pair.first().and_then(Value::as_str).map(str::to_owned)),
        _ => PluginEntry::Other,
    }
}

/// OpenCode 2 reading the legacy key: a spec, or exactly `[spec, {options}]`.
fn v2_legacy_plugin(item: &Value) -> PluginEntry {
    match item {
        Value::String(spec) => v2_spec(spec),
        Value::Array(pair) => match pair.as_slice() {
            [Value::String(spec), Value::Object(_)] => PluginEntry::WithOptions(Some(spec.clone())),
            _ => PluginEntry::Other,
        },
        _ => PluginEntry::Other,
    }
}

/// OpenCode 2 reading `plugins`: a spec, or `{package, options?}`.
fn v2_plugin(item: &Value) -> PluginEntry {
    let Value::Object(entry) = item else {
        return match item {
            Value::String(spec) => v2_spec(spec),
            _ => PluginEntry::Other,
        };
    };

    let Some(package) = entry.get("package").and_then(Value::as_str) else {
        return PluginEntry::Other;
    };

    match entry.get("options") {
        None => v2_spec(package),
        Some(Value::Object(_)) => PluginEntry::WithOptions(Some(package.to_owned())),
        Some(_) => PluginEntry::Other,
    }
}

/// A spec string; `-<target>` removes `<target>` (a lone `-` is an error OpenCode reports).
fn v2_spec(spec: &str) -> PluginEntry {
    match spec.strip_prefix('-') {
        Some("") => PluginEntry::Other,
        Some(target) => PluginEntry::Remove(target.to_owned()),
        None => PluginEntry::Spec(spec.to_owned()),
    }
}

/// The MCP servers of `document` by name, values as written; `None` when the server object is
/// not an object (`mcp`, or on OpenCode 2 `mcp.servers`). OpenCode 2 skips the legacy entries that
/// only set `enabled` and the `mcp.timeout` defaults.
pub(crate) fn mcp_servers(document: &Value, generation: OpenCodeGeneration) -> Option<Vec<(String, Value)>> {
    let mcp = match document.get("mcp") {
        None => return Some(Vec::new()),
        Some(Value::Object(mcp)) => mcp,
        Some(_) => return None,
    };

    if generation == OpenCodeGeneration::V1 {
        return Some(
            mcp.iter()
                .map(|(name, server)| (name.clone(), server.clone()))
                .collect(),
        );
    }

    let mut servers: Vec<(String, Value)> = Vec::new();
    let mut native: Option<&Map<String, Value>> = None;

    for (name, value) in mcp {
        if is_enabled_only(value) {
            continue;
        }

        let reserved = matches!(name.as_str(), "servers" | "timeout") && !is_direct_server(value);
        if !reserved {
            servers.push((name.clone(), value.clone()));
            continue;
        }

        if name == "servers" {
            native = Some(value.as_object()?);
        }
    }

    for (name, server) in native.into_iter().flatten() {
        replace(&mut servers, name, server.clone());
    }

    Some(servers)
}

/// A legacy server that only switches another layer's server on or off.
fn is_enabled_only(value: &Value) -> bool {
    value.get("type").is_none() && value.get("enabled").is_some_and(Value::is_boolean)
}

/// A legacy server named `servers` or `timeout` (it has a `type`).
fn is_direct_server(value: &Value) -> bool {
    matches!(value.get("type").and_then(Value::as_str), Some("local" | "remote"))
}

/// The commands of `document` by name: `command` on OpenCode 1, `command` then `commands` on
/// OpenCode 2.
pub(crate) fn command_entries(document: &Value, generation: OpenCodeGeneration) -> Vec<(String, Value)> {
    let mut commands = object_entries(document, "command");

    if generation == OpenCodeGeneration::V2 {
        for (name, command) in object_entries(document, "commands") {
            replace(&mut commands, &name, command);
        }
    }

    commands
}

/// The entries of the object at `key` (none for anything else).
pub(crate) fn object_entries(document: &Value, key: &str) -> Vec<(String, Value)> {
    let entries = document.get(key).and_then(Value::as_object).into_iter().flatten();

    entries.map(|(name, value)| (name.clone(), value.clone())).collect()
}

/// Sets `name` to `value`: in place when it is listed, else at the end.
pub(crate) fn replace<T>(entries: &mut Vec<(String, T)>, name: &str, value: T) {
    match entries.iter_mut().find(|(existing, _)| existing == name) {
        Some((_, existing)) => *existing = value,
        None => entries.push((name.to_owned(), value)),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::harness::OpenCodeGeneration::{V1, V2};

    #[test]
    fn opencode_1_reads_only_the_plugin_key() {
        let document = json!({ "plugin": ["a", ["b", { "k": 1 }], 3], "plugins": ["c"] });

        assert_eq!(
            plugin_entries(&document, V1),
            [
                PluginEntry::Spec("a".to_owned()),
                PluginEntry::WithOptions(Some("b".to_owned())),
                PluginEntry::Other,
            ]
        );
    }

    #[test]
    fn opencode_2_reads_plugin_then_plugins_with_removals() {
        let document = json!({
            "plugins": ["c", { "package": "d" }, { "package": "e", "options": {} }, "-a", "-", { "options": {} }],
            "plugin": ["a", ["b", { "k": 1 }], ["lonely"]],
        });

        assert_eq!(
            plugin_entries(&document, V2),
            [
                PluginEntry::Spec("a".to_owned()),
                PluginEntry::WithOptions(Some("b".to_owned())),
                PluginEntry::Other,
                PluginEntry::Spec("c".to_owned()),
                PluginEntry::Spec("d".to_owned()),
                PluginEntry::WithOptions(Some("e".to_owned())),
                PluginEntry::Remove("a".to_owned()),
                PluginEntry::Other,
                PluginEntry::Other,
            ]
        );
    }

    #[test]
    fn opencode_2_merges_legacy_and_nested_servers_with_the_nested_one_winning() {
        let document = json!({ "mcp": {
            "old": { "type": "local", "command": ["a"] },
            "toggle": { "enabled": false },
            "timeout": { "startup": 5 },
            "servers": {
                "old": { "type": "remote", "url": "https://x.example.com" },
                "new": { "type": "local", "command": ["b"] },
            },
        }});

        let names: Vec<String> = mcp_servers(&document, V2)
            .unwrap()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(names, ["old", "new"]);
        assert_eq!(mcp_servers(&document, V2).unwrap()[0].1["type"], "remote");

        let v1: Vec<String> = mcp_servers(&document, V1)
            .unwrap()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(v1, ["old", "toggle", "timeout", "servers"]);
    }

    #[test]
    fn servers_that_are_not_objects_are_reported() {
        assert_eq!(mcp_servers(&json!({ "mcp": [] }), V2), None);
        assert_eq!(mcp_servers(&json!({ "mcp": { "servers": 1 } }), V2), None);
        assert_eq!(mcp_servers(&json!({}), V2), Some(Vec::new()));
    }

    #[test]
    fn opencode_2_commands_override_legacy_ones_of_the_same_name() {
        let document = json!({
            "command": { "a": { "description": "old" }, "b": {} },
            "commands": { "a": { "description": "new" } },
        });

        let commands = command_entries(&document, V2);
        assert_eq!(commands[0], ("a".to_owned(), json!({ "description": "new" })));
        assert_eq!(commands.len(), 2);
        assert_eq!(command_entries(&document, V1)[0].1, json!({ "description": "old" }));
    }
}
