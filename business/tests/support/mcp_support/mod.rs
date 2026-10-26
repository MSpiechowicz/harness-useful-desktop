//! A temporary home with an OMP agent directory, for the tests that write a user MCP file.
//! Each test binary uses only some of these helpers.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use harness_business::env::Env;
use serde_json::{Value, json};
use tempfile::TempDir;

pub struct Home {
    pub dir: TempDir,
    pub env: Env,
}

impl Home {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::for_tests(dir.path());
        Self { dir, env }
    }

    /// `name` in `~/.omp/agent` (the directory itself for `""`).
    pub fn agent(&self, name: &str) -> PathBuf {
        self.dir.path().join(".omp/agent").join(name)
    }

    pub fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.agent(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    /// Writes [`with_secrets`] as `mcp.json` (mode `0640` on Unix) and returns what was written
    /// and where.
    pub fn write_with_secrets(&self, disabled_servers: Value) -> (Value, PathBuf) {
        let original = with_secrets(disabled_servers);
        let path = self.write("mcp.json", &serde_json::to_string(&original).unwrap());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        }

        (original, path)
    }

    /// Makes `mcp.json` a symlink to `~/dotfiles/mcp.json` (holding `{}`) and returns the target.
    #[cfg(unix)]
    pub fn symlink_mcp_file(&self) -> PathBuf {
        let real = self.dir.path().join("dotfiles/mcp.json");
        std::fs::create_dir_all(real.parent().unwrap()).unwrap();
        std::fs::write(&real, "{}").unwrap();
        std::fs::create_dir_all(self.agent("")).unwrap();
        std::os::unix::fs::symlink(&real, self.agent("mcp.json")).unwrap();
        real
    }

    /// Every file name in the agent directory, sorted.
    pub fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.agent(""))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

/// An MCP file with two servers holding secrets, `disabledServers` and other keys, in an order
/// that must be kept.
pub fn with_secrets(disabled_servers: Value) -> Value {
    json!({
        "$schema": "https://example.com/mcp.schema.json",
        "mcpServers": {
            "github": {
                "command": "github-mcp",
                "env": { "GITHUB_TOKEN": "CANARY-mcp-token-7f3a9c" }
            },
            "search": {
                "url": "https://search.example.com/mcp",
                "headers": { "Authorization": "Bearer abc.def.ghi" }
            }
        },
        "disabledServers": disabled_servers,
        "zLast": { "nested": [1, 2.5, "three"] }
    })
}

pub fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

pub fn keys(value: &Value) -> Vec<String> {
    value.as_object().unwrap().keys().cloned().collect()
}

#[cfg(unix)]
pub fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}
