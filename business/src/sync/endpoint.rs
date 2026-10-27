//! Whether an existing MCP entry reaches the same endpoint as a shared server, which decides what a
//! replace may keep: the existing entry's own settings (env, headers, OAuth, ...) belong to the
//! server they were set up for, so they are only kept for the same endpoint. For another one the
//! whole entry is replaced by what a new add writes.
//!
//! Only an HTTP or SSE server has an endpoint that tells the existing entry's settings are meant for
//! it: the same transport and the same scheme, host and port. A stdio server never does: what it
//! connects to hangs on arguments, a package or an image (`mcp-remote <url>`, `docker run <image>`,
//! `--api-url=...`) that a shared entry controls, so env of the old entry could reach another
//! destination. Any stdio replace, and any change of transport, replaces the whole entry.

use serde_json::Value;

use super::schema::{McpServer, Transport};
use crate::ecosystem::Shape;

/// The transport of `existing` as `shape` writes it, `None` when it is none we know.
fn transport_of(existing: &Value, shape: Shape) -> Option<Transport> {
    let fields = existing.as_object()?;
    let kind = fields.get("type").map(Value::as_str);

    if shape.is_opencode() {
        return match kind {
            Some(Some("local")) => Some(Transport::Stdio),
            Some(Some("remote")) => Some(Transport::Http),
            _ => None,
        };
    }

    match kind {
        Some(Some("stdio")) => Some(Transport::Stdio),
        Some(Some("http" | "streamable-http")) => Some(Transport::Http),
        Some(Some("sse")) => Some(Transport::Sse),
        Some(_) => None,
        None if fields.contains_key("command") => Some(Transport::Stdio),
        None if fields.contains_key("url") => Some(Transport::Http),
        None => None,
    }
}

/// True when `existing` (an entry of a `shape` file) is an HTTP or SSE server at the same scheme,
/// host and port as `shared`, which is HTTP or SSE too. Never true for stdio.
pub(super) fn same_endpoint(existing: &Value, shared: &McpServer, shape: Shape) -> bool {
    let remote = |transport| matches!(transport, Transport::Http | Transport::Sse);
    let Some(transport) = transport_of(existing, shape).filter(|transport| remote(*transport)) else {
        return false;
    };

    // OpenCode's `remote` covers HTTP and SSE.
    let alike = transport == shared.transport || (shape.is_opencode() && remote(shared.transport));
    let url = existing.get("url").and_then(Value::as_str);

    alike
        && matches!((url.and_then(origin), shared.url.as_deref().and_then(origin)), (Some(left), Some(right)) if left == right)
}

/// The scheme, host and port of `url` (the scheme's default port when it has none), in lower case.
fn origin(url: &str) -> Option<(String, String, u16)> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, host)| host);

    let (host, port) = match host_port.strip_prefix('[') {
        Some(inner) => {
            let (host, after) = inner.split_once(']')?;
            (host, after.strip_prefix(':'))
        }
        None => match host_port.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (host_port, None),
        },
    };

    let default = match scheme.as_str() {
        "http" => 80,
        "https" => 443,
        _ => return None,
    };
    let port = match port {
        Some(port) => port.parse::<u16>().ok()?,
        None => default,
    };

    if host.is_empty() {
        return None;
    }

    Some((scheme, host.to_ascii_lowercase(), port))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn server(fields: Value) -> McpServer {
        serde_json::from_value(fields).expect("a server")
    }

    fn http(url: &str) -> McpServer {
        server(json!({ "name": "s", "transport": "http", "url": url }))
    }

    fn stdio(command: &str, args: &[&str]) -> McpServer {
        server(json!({ "name": "s", "transport": "stdio", "command": command, "args": args }))
    }

    #[test]
    fn an_http_endpoint_is_scheme_host_and_port() {
        let shared = http("https://mcp.example.com/v2");
        let at = |url: &str| json!({ "type": "http", "url": url });

        assert!(same_endpoint(
            &at("https://MCP.example.com:443/other?x=1"),
            &shared,
            Shape::McpServers
        ));
        assert!(!same_endpoint(
            &at("https://evil.example.com/v2"),
            &shared,
            Shape::McpServers
        ));
        assert!(!same_endpoint(
            &at("http://mcp.example.com/v2"),
            &shared,
            Shape::McpServers
        ));
        assert!(!same_endpoint(
            &at("https://mcp.example.com:8443/v2"),
            &shared,
            Shape::McpServers
        ));
        assert!(!same_endpoint(&at("not a url"), &shared, Shape::McpServers));
    }

    #[test]
    fn a_transport_change_is_another_endpoint() {
        let shared = http("https://mcp.example.com/v2");

        assert!(!same_endpoint(
            &json!({ "command": "npx", "args": [] }),
            &shared,
            Shape::McpServers
        ));
        assert!(!same_endpoint(
            &json!({ "type": "sse", "url": "https://mcp.example.com/v2" }),
            &shared,
            Shape::McpServers
        ));
        assert!(same_endpoint(
            &json!({ "type": "remote", "url": "https://mcp.example.com/x" }),
            &shared,
            Shape::OpenCode
        ));
        assert!(!same_endpoint(
            &json!({ "type": "local", "command": ["x"] }),
            &shared,
            Shape::OpenCode
        ));
    }

    #[test]
    fn a_stdio_server_never_has_the_same_endpoint() {
        let shared = stdio("npx", &["-y", "@acme/server", "--flag"]);

        for existing in [
            json!({ "command": "npx", "args": ["-y", "@acme/server", "--flag"] }),
            json!({ "command": "npx", "args": ["@acme/server"] }),
            json!({ "type": "local", "command": ["npx", "-y", "@acme/server"] }),
        ] {
            let shape = if existing.get("type").is_some() {
                Shape::OpenCode
            } else {
                Shape::McpServers
            };

            assert!(!same_endpoint(&existing, &shared, shape), "{existing}");
        }
    }
}
