//! Where a [`Shape`] of MCP file keeps its servers, and the entry the app writes into it.
//!
//! OpenCode 2 reads `mcp.servers.<name>` and normalizes the OpenCode 1 servers `mcp.<name>` of the
//! same file into it (`core/src/config/normalize.ts`, 2.1.2): a nested server replaces a legacy
//! one of the same name, legacy entries that only set `enabled` are dropped, and `mcp.timeout`
//! holds defaults unless it is a server itself. Lookups see both ([`servers`]); new entries go
//! into `mcp.servers` ([`servers_mut`]).

use serde_json::{Map, Value, json};

use crate::harness::OpenCodeGeneration;
use crate::harnesses::opencode::mcp_servers;

/// The `$schema` OpenCode writes into its config files (OpenCode 2 accepts it too).
const OPENCODE_SCHEMA: &str = "https://opencode.ai/config.json";

/// The key under `mcp` holding OpenCode 2's servers.
const NESTED: &str = "servers";

/// How an MCP file holds its servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// `{"mcpServers": {"<name>": {"command": "<binary>", "args": ["mcp"]}}}`, strict JSON (OMP's and
    /// PI's `mcp.json`).
    McpServers,
    /// `{"mcp": {"<name>": {"type": "local", "command": ["<binary>", "mcp"], "enabled": true}}}`,
    /// JSONC (OpenCode 1's config file).
    OpenCode,
    /// `{"mcp": {"servers": {"<name>": {"type": "local", "command": ["<binary>", "mcp"]}}}}`, JSONC
    /// (OpenCode 2's config file, which also reads the OpenCode 1 servers `mcp.<name>`). A server
    /// is switched off with `"disabled": true`; OpenCode 2 has no `enabled`.
    OpenCodeV2,
}

impl Shape {
    /// The top-level key holding the servers (OpenCode 2 nests them one level deeper).
    pub fn root(self) -> &'static str {
        match self {
            Self::McpServers => "mcpServers",
            Self::OpenCode | Self::OpenCodeV2 => "mcp",
        }
    }

    /// The entry that runs `<command> mcp`.
    pub fn entry(self, command: &str) -> Value {
        match self {
            Self::McpServers => json!({ "command": command, "args": ["mcp"] }),
            Self::OpenCode => json!({ "type": "local", "command": [command, "mcp"], "enabled": true }),
            Self::OpenCodeV2 => json!({ "type": "local", "command": [command, "mcp"] }),
        }
    }

    /// Whether the file is an OpenCode config file (JSONC, `command` arrays).
    pub fn is_opencode(self) -> bool {
        matches!(self, Self::OpenCode | Self::OpenCodeV2)
    }

    /// Whether `server` is switched off by its own fields: `"enabled": false`, or on OpenCode 2
    /// also `"disabled": true` (a legacy `enabled: false` becomes that).
    pub fn switched_off(self, server: &Value) -> bool {
        let disabled = self == Self::OpenCodeV2 && server.get("disabled") == Some(&Value::Bool(true));

        disabled || server.get("enabled") == Some(&Value::Bool(false))
    }

    /// The document a missing file starts as.
    pub(super) fn new_document(self) -> Map<String, Value> {
        let mut document = Map::new();

        if self.is_opencode() {
            document.insert("$schema".to_owned(), Value::String(OPENCODE_SCHEMA.to_owned()));
        }

        document
    }

    /// Whether the servers of `document` can be read and added to: the root key is missing or an
    /// object, and on OpenCode 2 `mcp.servers` is missing or an object of servers (a legacy server
    /// named `servers` is refused, since new servers would land inside it).
    pub(super) fn servers_valid(self, document: &Map<String, Value>) -> bool {
        let root = match document.get(self.root()) {
            None => return true,
            Some(Value::Object(root)) => root,
            Some(_) => return false,
        };

        if self != Self::OpenCodeV2 {
            return true;
        }

        match root.get(NESTED) {
            None => true,
            Some(Value::Object(nested)) => !is_direct_server(nested),
            Some(_) => false,
        }
    }
}

/// The servers of `document` by name, as the harness sees them; `None` when there is no servers
/// object. On OpenCode 2 the legacy `mcp.<name>` servers come first and a nested one replaces a
/// legacy one of the same name.
pub(crate) fn servers(document: &Map<String, Value>, shape: Shape) -> Option<Map<String, Value>> {
    let root = document.get(shape.root())?;

    if shape != Shape::OpenCodeV2 {
        return root.as_object().cloned();
    }

    let file = json!({ "mcp": root });
    let merged = mcp_servers(&file, OpenCodeGeneration::V2)?;

    Some(merged.into_iter().collect())
}

/// The servers object new entries go into, created when missing: the root object, on OpenCode 2
/// `mcp.servers`. `None` when a key on the way holds something else (which `load` refuses).
pub(crate) fn servers_mut(document: &mut Map<String, Value>, shape: Shape) -> Option<&mut Map<String, Value>> {
    let root = document
        .entry(shape.root())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()?;

    if shape != Shape::OpenCodeV2 {
        return Some(root);
    }

    root.entry(NESTED)
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
}

/// The server `name` as [`servers`] sees it, to change in place: on OpenCode 2 the nested one
/// when there is one, else the legacy one.
pub(crate) fn server_mut<'a>(document: &'a mut Map<String, Value>, shape: Shape, name: &str) -> Option<&'a mut Value> {
    let legacy = shape == Shape::OpenCodeV2 && is_legacy_only(document, name);
    let root = document.get_mut(shape.root())?.as_object_mut()?;

    if shape != Shape::OpenCodeV2 || legacy {
        return root.get_mut(name);
    }

    root.get_mut(NESTED)?.as_object_mut()?.get_mut(name)
}

/// Removes the server `name` when `owned` accepts it; true when one was removed. On OpenCode 2
/// both copies are checked: the nested one, then a legacy `mcp.<name>` server that would show
/// through once the nested one is gone. A copy `owned` refuses is kept as it is (OpenCode then
/// still sees it, see [`servers`]).
pub(crate) fn remove_server(
    document: &mut Map<String, Value>,
    shape: Shape,
    name: &str,
    owned: impl Fn(&Value) -> bool,
) -> bool {
    let Some(root) = document.get_mut(shape.root()).and_then(Value::as_object_mut) else {
        return false;
    };

    if shape != Shape::OpenCodeV2 {
        return remove_owned(root, name, &owned);
    }

    let nested = root
        .get_mut(NESTED)
        .and_then(Value::as_object_mut)
        .is_some_and(|nested| remove_owned(nested, name, &owned));

    let legacy = is_legacy_only(document, name)
        && document
            .get_mut(shape.root())
            .and_then(Value::as_object_mut)
            .is_some_and(|root| remove_owned(root, name, &owned));

    nested || legacy
}

/// Removes `servers[name]` when `owned` accepts it; true when it was removed.
fn remove_owned(servers: &mut Map<String, Value>, name: &str, owned: impl Fn(&Value) -> bool) -> bool {
    if !servers.get(name).is_some_and(owned) {
        return false;
    }

    servers.shift_remove(name).is_some()
}

/// True when OpenCode 2 sees `name` only as a legacy `mcp.<name>` server.
fn is_legacy_only(document: &Map<String, Value>, name: &str) -> bool {
    let nested = document
        .get("mcp")
        .and_then(|mcp| mcp.get(NESTED))
        .and_then(Value::as_object)
        .is_some_and(|nested| nested.contains_key(name));

    !nested && servers(document, Shape::OpenCodeV2).is_some_and(|servers| servers.contains_key(name))
}

/// An object that is a server itself (OpenCode 2 reads `mcp.servers` as one then).
fn is_direct_server(object: &Map<String, Value>) -> bool {
    matches!(object.get("type").and_then(Value::as_str), Some("local" | "remote"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap()
    }

    fn any(_: &Value) -> bool {
        true
    }

    #[test]
    fn opencode_2_sees_legacy_and_nested_servers_and_adds_nested_ones() {
        let mut file = document(json!({ "mcp": {
            "old": { "type": "local", "command": ["a"] },
            "toggle": { "enabled": false },
            "servers": { "new": { "type": "local", "command": ["b"] } },
        }}));

        let names: Vec<String> = servers(&file, Shape::OpenCodeV2).unwrap().keys().cloned().collect();
        assert_eq!(names, ["old", "new"]);

        servers_mut(&mut file, Shape::OpenCodeV2)
            .unwrap()
            .insert("added".to_owned(), json!({}));
        assert!(file["mcp"]["servers"].get("added").is_some());

        assert!(server_mut(&mut file, Shape::OpenCodeV2, "old").is_some());
        assert!(remove_server(&mut file, Shape::OpenCodeV2, "old", any));
        assert!(file["mcp"].get("old").is_none());
        assert!(remove_server(&mut file, Shape::OpenCodeV2, "new", any));
        assert!(file["mcp"]["servers"].get("new").is_none());
        assert!(
            !remove_server(&mut file, Shape::OpenCodeV2, "toggle", any),
            "not a server on OpenCode 2"
        );
    }

    #[test]
    fn a_nested_server_wins_over_a_legacy_one_of_the_same_name() {
        let mut file = document(json!({ "mcp": {
            "same": { "type": "local", "command": ["legacy"] },
            "servers": { "same": { "type": "local", "command": ["nested"] } },
        }}));

        assert_eq!(
            servers(&file, Shape::OpenCodeV2).unwrap()["same"]["command"][0],
            "nested"
        );
        let found = server_mut(&mut file, Shape::OpenCodeV2, "same").unwrap();
        assert_eq!(found["command"][0], "nested");
    }

    #[test]
    fn removing_a_server_on_opencode_2_removes_its_nested_and_legacy_copies() {
        let mut file = document(json!({ "mcp": {
            "same": { "type": "local", "command": ["legacy"] },
            "toggle": { "enabled": false },
            "servers": {
                "same": { "type": "local", "command": ["nested"] },
                "toggle": { "type": "local", "command": ["t"] },
            },
        }}));

        assert!(remove_server(&mut file, Shape::OpenCodeV2, "same", any));
        assert!(file["mcp"].get("same").is_none());
        assert!(file["mcp"]["servers"].get("same").is_none());
        assert!(servers(&file, Shape::OpenCodeV2).unwrap().get("same").is_none());

        assert!(remove_server(&mut file, Shape::OpenCodeV2, "toggle", any));
        assert_eq!(
            file["mcp"]["toggle"],
            json!({ "enabled": false }),
            "a legacy toggle is not a server on OpenCode 2"
        );
        assert!(!remove_server(&mut file, Shape::OpenCodeV2, "servers", any));
        assert!(file["mcp"].get("servers").is_some());
    }

    #[test]
    fn a_copy_that_is_not_owned_is_kept() {
        let mine = |server: &Value| server["command"][0] == "mine";
        let legacy =
            json!({ "type": "remote", "url": "https://team.example/mcp", "headers": { "Authorization": "x" } });
        let mut file = document(json!({ "mcp": {
            "same": legacy,
            "servers": { "same": { "type": "local", "command": ["mine"] } },
        }}));

        assert!(remove_server(&mut file, Shape::OpenCodeV2, "same", mine));
        assert!(file["mcp"]["servers"].get("same").is_none());
        assert_eq!(file["mcp"]["same"], legacy);
        assert_eq!(servers(&file, Shape::OpenCodeV2).unwrap()["same"], legacy);

        assert!(!remove_server(&mut file, Shape::OpenCodeV2, "same", mine));
        assert_eq!(file["mcp"]["same"], legacy);

        let mut flat = document(json!({ "mcpServers": { "same": { "command": "other" } } }));
        assert!(!remove_server(&mut flat, Shape::McpServers, "same", mine));
        assert!(flat["mcpServers"].get("same").is_some());
    }

    #[test]
    fn servers_that_cannot_take_a_nested_entry_are_invalid() {
        let valid = |value: Value| Shape::OpenCodeV2.servers_valid(&document(value));

        assert!(valid(json!({})));
        assert!(valid(json!({ "mcp": { "servers": {} } })));
        assert!(!valid(json!({ "mcp": [] })));
        assert!(!valid(json!({ "mcp": { "servers": [] } })));
        assert!(!valid(
            json!({ "mcp": { "servers": { "type": "local", "command": ["x"] } } })
        ));
        assert!(Shape::OpenCode.servers_valid(&document(json!({ "mcp": { "servers": [] } }))));
    }

    #[test]
    fn opencode_2_switches_a_server_off_with_disabled() {
        assert!(Shape::OpenCodeV2.switched_off(&json!({ "disabled": true })));
        assert!(Shape::OpenCodeV2.switched_off(&json!({ "enabled": false })));
        assert!(!Shape::OpenCode.switched_off(&json!({ "disabled": true })));
        assert!(Shape::OpenCode.switched_off(&json!({ "enabled": false })));
    }
}
