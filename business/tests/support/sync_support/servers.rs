//! MCP server builders for the "Share setup" tests.
//!
//! Each helper is used by some tests only, hence the `dead_code` allowance.
#![allow(dead_code)]

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

/// The GitHub server the export tests share: `npx -y @modelcontextprotocol/server-github` with a
/// `GITHUB_TOKEN` env key.
pub fn github_server() -> McpServer {
    McpServer {
        env_keys: vec!["GITHUB_TOKEN".to_owned()],
        ..stdio("github", "npx", &["-y", "@modelcontextprotocol/server-github"])
    }
}
