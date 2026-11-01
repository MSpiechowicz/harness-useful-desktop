//! What tells one Harness family server (`harness-memory`, `harness-knowledge`) from another.
//!
//! The modules named `sidecar_*` do the work the app does for every such server (find its
//! binary, run its commands, probe what it lists, keep its embedder running, edit its config
//! file, register it with the harnesses) and take a [`Sidecar`] for what differs: its names. A
//! product's own module (`memory_binary`, `memory_cli`, ...) keeps its [`Sidecar`] constant and the
//! parts that only it has.

use std::path::PathBuf;

use crate::app_dirs::shared_data_dir;
use crate::env::Env;

/// The names of one server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sidecar {
    /// The executable's name without `.exe`, also the server's folder in the config and the data
    /// folder of the platform (`harness-memory`).
    pub binary: &'static str,
    /// The name in messages shown to the user (`Harness Memory`).
    pub product: &'static str,
    /// The name the app gives the server in a harness's `mcpServers` (`harness-memory`).
    pub mcp_server: &'static str,
    /// The prefix of the server's environment variables, without the underscore
    /// (`HARNESS_MEMORY`).
    pub env_prefix: &'static str,
    /// What the app's switch for the server is called in log lines (`project memory`).
    pub feature: &'static str,
}

impl Sidecar {
    /// The server's environment variable `HARNESS_MEMORY_<suffix>`.
    pub fn var(&self, suffix: &str) -> String {
        format!("{}_{suffix}", self.env_prefix)
    }

    /// The server's data folder, as it finds it (see [`shared_data_dir`]): `<prefix>_DIR` when set
    /// to an absolute path, else the platform's data folder followed by [`Self::binary`].
    pub fn data_dir(&self, env: &Env) -> Option<PathBuf> {
        shared_data_dir(env, &self.var("DIR"), self.binary)
    }
}
