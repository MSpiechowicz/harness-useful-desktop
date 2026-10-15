//! MCP server builders for the "Share setup" schema tests.

use harness_business::sync::schema::{McpServer, Transport};

/// A stdio server running `command args…`, with no env or header keys.
pub fn stdio(name: &str, command: &str, args: &[&str]) -> McpServer {
    McpServer {
        name: name.to_owned(),
        transport: Transport::Stdio,
        command: Some(command.to_owned()),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        url: None,
        env_keys: Vec::new(),
        header_keys: Vec::new(),
    }
}

/// An HTTP server at `url`, with no env or header keys.
pub fn http(name: &str, url: &str) -> McpServer {
    McpServer {
        name: name.to_owned(),
        transport: Transport::Http,
        command: None,
        args: Vec::new(),
        url: Some(url.to_owned()),
        env_keys: Vec::new(),
        header_keys: Vec::new(),
    }
}
