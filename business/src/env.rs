//! Injected process environment.
//!
//! All path logic takes an [`Env`] instead of reading `std::env` or `dirs` directly, so tests run
//! against fixture homes and never touch the real `~`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::error::{BusinessError, Result};

/// Environment variables captured by [`Env::from_process`]. Nothing else is read.
pub const CAPTURED_VARS: &[&str] = &[
    "PATH",
    "APPDATA",
    "LOCALAPPDATA",
    "USERPROFILE",
    // The Windows directory, where the dashboard installer finds Windows PowerShell.
    "SystemRoot",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_STATE_HOME",
    "XDG_CACHE_HOME",
    "PI_CONFIG_DIR",
    "PI_CODING_AGENT_DIR",
    "PI_CODING_AGENT_SESSION_DIR",
    // The temp root (`os.tmpdir()`), which OMP session folder names are relative to.
    "TMPDIR",
    "TEMP",
    "TMP",
    "OMP_PROFILE",
    "PI_PROFILE",
    "OPENCODE_CONFIG",
    "OPENCODE_CONFIG_DIR",
    "OPENCODE_DISABLE_PROJECT_CONFIG",
    // OpenCode 2's name for the switch above, which it reads first.
    "OPENCODE_CONFIG_PROJECT_DISABLE",
    // The Windows folder of OpenCode 2's system config directory (`%ProgramData%\opencode`).
    "ProgramData",
    // The Windows folder of Claude Code's managed settings (`%ProgramFiles%\ClaudeCode`).
    "ProgramFiles",
    // Claude Code's config directory, which also moves its global `.claude.json`.
    "CLAUDE_CONFIG_DIR",
    // Claude Code's cloud provider switches (not secret, shown as set).
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
    // Harness Memory's data folder, where the app meets the server for script run requests.
    "HARNESS_MEMORY_DIR",
    // Harness Memory's config file, when not the default one (the team database settings).
    "HARNESS_MEMORY_CONFIG",
    // The hooks folder, shared by the app window and its `harness-desktop mcp` mode.
    "HARNESS_HOOKS_DIR",
    // The routines folder, shared the same way.
    "HARNESS_ROUTINES_DIR",
];

/// Variables whose presence [`Env::from_process`] records without their value: they may hold
/// secrets (inline OpenCode config and permissions, Claude Code's API keys, tokens and custom
/// headers), and the app only reports that they are set.
pub const PRESENCE_VARS: &[&str] = &[
    "OPENCODE_PERMISSION",
    "OPENCODE_CONFIG_CONTENT",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
    "ANTHROPIC_CUSTOM_HEADERS",
    "AWS_BEARER_TOKEN_BEDROCK",
    "ANTHROPIC_FOUNDRY_API_KEY",
    "ANTHROPIC_FOUNDRY_AUTH_TOKEN",
];

/// Operating-system family, used for platform-specific paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsKind {
    /// Linux and other non-Apple Unix systems.
    Linux,
    /// macOS.
    MacOs,
    /// Windows.
    Windows,
}

impl OsKind {
    /// The OS this binary was built for.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// Snapshot of the environment the business layer depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Env {
    /// The user's home directory.
    pub home: PathBuf,
    /// Captured variables (only [`CAPTURED_VARS`] in production).
    pub vars: BTreeMap<String, String>,
    /// The [`PRESENCE_VARS`] that are set to a non-empty value; their values are never kept.
    pub present: BTreeSet<String>,
    /// OS family.
    pub os: OsKind,
}

impl Env {
    /// Captures the real environment once at startup.
    pub fn from_process() -> Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| BusinessError::NotFound("home directory".to_owned()))?;

        let vars = CAPTURED_VARS
            .iter()
            .filter_map(|name| std::env::var(name).ok().map(|value| ((*name).to_owned(), value)))
            .collect();

        let present = PRESENCE_VARS
            .iter()
            .filter(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
            .map(|name| (*name).to_owned())
            .collect();

        let env = Self {
            home,
            vars,
            present,
            os: OsKind::current(),
        };

        env.guard_private_roots();
        Ok(env)
    }

    /// An environment rooted at `home` with no variables and the current OS (for tests).
    pub fn for_tests(home: impl Into<PathBuf>) -> Self {
        Self {
            home: home.into(),
            vars: BTreeMap::new(),
            present: BTreeSet::new(),
            os: OsKind::current(),
        }
    }

    /// Returns a copy with `name` set to `value`. For a [`PRESENCE_VARS`] name only its presence is
    /// recorded, as in production.
    pub fn with_var(mut self, name: &str, value: impl Into<String>) -> Self {
        let value = value.into();

        if PRESENCE_VARS.contains(&name) {
            if value.is_empty() {
                self.present.remove(name);
            } else {
                self.present.insert(name.to_owned());
            }

            return self;
        }

        self.vars.insert(name.to_owned(), value);
        self.guard_private_roots();
        self
    }

    /// Adds the config directory an absolute `CLAUDE_CONFIG_DIR` names to the deny-list's Claude
    /// roots (see [`crate::security::deny::add_claude_root`]), so its private files stay unread
    /// whatever the directory is called.
    fn guard_private_roots(&self) {
        let claude_root = self
            .var("CLAUDE_CONFIG_DIR")
            .map(Path::new)
            .filter(|dir| dir.is_absolute());

        if let Some(root) = claude_root {
            crate::security::deny::add_claude_root(root);
        }
    }

    /// Returns a copy that reports `os` (for platform-specific path tests).
    pub fn with_os(mut self, os: OsKind) -> Self {
        self.os = os;
        self
    }

    /// A captured variable, or `None` when unset or empty.
    pub fn var(&self, name: &str) -> Option<&str> {
        self.vars
            .get(name)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    /// True when the presence-only variable `name` (see [`PRESENCE_VARS`]) is set to a non-empty
    /// value.
    pub fn is_set(&self, name: &str) -> bool {
        self.present.contains(name)
    }

    /// Expands a leading `~` (alone, or followed by `/` or `\`) to the home directory.
    pub fn expand_home(&self, raw: &str) -> PathBuf {
        if raw == "~" {
            return self.home.clone();
        }

        match raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
            Some(rest) => self.home.join(rest),
            None => PathBuf::from(raw),
        }
    }

    /// `home.join(relative)`, for readability at call sites. `relative` may use `/` on every OS; it is
    /// rebuilt from its components, so on Windows the result uses `\` throughout.
    pub fn home_join(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.home.join(relative.as_ref().components().collect::<PathBuf>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_presence_variable_records_only_that_it_is_set() {
        let env = Env::for_tests("/home/u").with_var("OPENCODE_PERMISSION", r#"{"bash":"allow"}"#);

        assert!(env.is_set("OPENCODE_PERMISSION"));
        assert!(!env.is_set("OPENCODE_CONFIG_CONTENT"));
        assert_eq!(env.var("OPENCODE_PERMISSION"), None);
        assert!(env.vars.is_empty());
    }

    #[test]
    fn an_empty_presence_variable_counts_as_unset() {
        let env = Env::for_tests("/home/u")
            .with_var("OPENCODE_CONFIG_CONTENT", "{}")
            .with_var("OPENCODE_CONFIG_CONTENT", "");

        assert!(!env.is_set("OPENCODE_CONFIG_CONTENT"));
    }
}
