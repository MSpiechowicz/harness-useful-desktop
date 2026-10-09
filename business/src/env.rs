//! Injected process environment.
//!
//! All path logic takes an [`Env`] instead of reading `std::env` or `dirs` directly, so tests run
//! against fixture homes and never touch the real `~`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{BusinessError, Result};

/// Environment variables captured by [`Env::from_process`]. Nothing else is read.
pub const CAPTURED_VARS: &[&str] = &[
    "PATH",
    "APPDATA",
    "LOCALAPPDATA",
    "USERPROFILE",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "PI_CONFIG_DIR",
    "PI_CODING_AGENT_DIR",
    "PI_CODING_AGENT_SESSION_DIR",
    // The temp root (`os.tmpdir()`), which OMP session folder names are relative to.
    "TMPDIR",
    "TEMP",
    "TMP",
    "OMP_PROFILE",
    "PI_PROFILE",
    "HARNESS_DASHBOARD_HOME",
    "HARNESS_DASHBOARD_DB",
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

        Ok(Self {
            home,
            vars,
            os: OsKind::current(),
        })
    }

    /// An environment rooted at `home` with no variables and the current OS (for tests).
    pub fn for_tests(home: impl Into<PathBuf>) -> Self {
        Self {
            home: home.into(),
            vars: BTreeMap::new(),
            os: OsKind::current(),
        }
    }

    /// Returns a copy with `name` set to `value`.
    pub fn with_var(mut self, name: &str, value: impl Into<String>) -> Self {
        self.vars.insert(name.to_owned(), value.into());
        self
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

    /// `home.join(relative)`, for readability at call sites.
    pub fn home_join(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.home.join(relative)
    }
}
