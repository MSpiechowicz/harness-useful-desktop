//! Replacing an existing server entry in place. When the caller says the existing entry is an HTTP or
//! SSE server at the same endpoint as the new one, only its managed keys change (see [`Shape::managed_keys`]),
//! every other key keeps its value and its position; otherwise the whole entry is swapped for the
//! new one (it keeps its position), so nothing set up for another server survives.

use serde_json::{Map, Value};

use super::Shape;

/// What [`super::merge_entries`] does with one entry.
pub(crate) enum EntryChange<'a> {
    /// Adds the definition under a free name; an existing one is never touched.
    Add(Value),
    /// Adds the definition under a free name, else replaces the existing entry: when `keeps` says
    /// the existing entry is the same endpoint, its managed keys are set to those of the
    /// definition (see [`replace_managed`]), otherwise the whole entry becomes the definition.
    /// `keeps` is asked about the entry as read in the same pass that writes it.
    Replace {
        definition: Value,
        keeps: &'a dyn Fn(&Value) -> bool,
    },
}

impl EntryChange<'_> {
    pub(super) fn definition(&self) -> &Value {
        match self {
            Self::Add(definition) | Self::Replace { definition, .. } => definition,
        }
    }
}

/// The managed keys of `entry` with their values, in the entry's order.
pub(crate) fn managed_part(entry: &Value, shape: Shape) -> Map<String, Value> {
    let Some(fields) = entry.as_object() else {
        return Map::new();
    };

    let keys = shape.managed_keys();

    fields
        .iter()
        .filter(|(key, _)| keys.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// Sets the managed keys of `existing` to those of `definition`: a key both have keeps its position
/// in `existing`, one only `definition` has is added at the end, one only `existing` has is removed
/// (a transport change). Nothing else changes. True when `existing` changed.
pub(crate) fn replace_managed(existing: &mut Value, definition: &Value, shape: Shape) -> bool {
    let wanted = managed_part(definition, shape);

    if managed_part(existing, shape) == wanted {
        return false;
    }

    let Some(fields) = existing.as_object_mut() else {
        return false;
    };

    for key in shape.managed_keys() {
        if !wanted.contains_key(*key) {
            fields.shift_remove(*key);
        }
    }

    for (key, value) in wanted {
        fields.insert(key, value);
    }

    true
}

/// The managed fields of `definition` as `"key": value` lines ready to paste over those of an
/// existing entry: nothing but what an import sets (no `enabled`, `disabled`, `env`, ...).
pub(crate) fn managed_text(definition: &Value, shape: Shape) -> String {
    managed_part(definition, shape)
        .iter()
        .map(|(key, value)| format!("{}: {value}", Value::String(key.clone())))
        .collect::<Vec<_>>()
        .join(",\n")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn only_managed_keys_change_and_keep_their_place() {
        let mut existing = json!({ "command": "old", "env": { "K": "v" }, "args": ["a"], "cwd": "/x" });
        let definition = json!({ "command": "new", "args": ["b"], "enabled": false });

        assert!(replace_managed(&mut existing, &definition, Shape::McpServers));
        assert_eq!(
            existing,
            json!({ "command": "new", "env": { "K": "v" }, "args": ["b"], "cwd": "/x" })
        );
        assert_eq!(
            existing.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["command", "env", "args", "cwd"]
        );
    }

    #[test]
    fn a_transport_change_swaps_the_managed_keys_and_keeps_the_rest() {
        let mut existing = json!({ "command": "old", "args": [], "env": { "K": "v" }, "enabled": false });

        assert!(replace_managed(
            &mut existing,
            &json!({ "type": "http", "url": "https://x.example/mcp" }),
            Shape::McpServers
        ));
        assert_eq!(
            existing,
            json!({ "env": { "K": "v" }, "enabled": false, "type": "http", "url": "https://x.example/mcp" })
        );
    }

    #[test]
    fn an_identical_managed_part_changes_nothing() {
        let mut existing = json!({ "command": "a", "args": [], "env": { "K": "v" } });

        assert!(!replace_managed(
            &mut existing,
            &json!({ "command": "a", "args": [] }),
            Shape::McpServers
        ));
    }

    #[test]
    fn opencode_manages_type_command_and_url_only() {
        let mut existing = json!({ "type": "local", "command": ["a"], "enabled": false, "environment": { "K": "v" } });
        let definition = json!({ "type": "local", "command": ["b", "c"], "enabled": true });

        assert!(replace_managed(&mut existing, &definition, Shape::OpenCode));
        assert_eq!(
            existing,
            json!({ "type": "local", "command": ["b", "c"], "enabled": false, "environment": { "K": "v" } })
        );
    }

    #[test]
    fn the_text_holds_the_managed_fields_only() {
        let definition = json!({ "type": "local", "command": ["b", "c"], "enabled": false });

        assert_eq!(
            managed_text(&definition, Shape::OpenCode),
            "\"type\": \"local\",\n\"command\": [\"b\",\"c\"]"
        );
    }
}
