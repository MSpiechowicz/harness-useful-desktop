//! What the installed Harness Memory can do beyond its commands: `harness-memory features` prints
//! one feature per line and exits 0. The desktop reads seven:
//!
//! - `notes-remote-local`: a note scope may be routed `remote+local` (read the team database and
//!   this computer, save here);
//! - `notes-hide`: `notes hide|unhide`, `notes list|get --hidden` and `notes discard-local`;
//! - `push-json`: `push --json` with `--overwrite`, `--delete-copied` and `--scope`;
//! - `scripts-remote-local`: scripts may be routed `remote+local`;
//! - `scripts-hide`: `scripts hide|unhide`, `scripts list|get --hidden` and `scripts
//!   discard-local`;
//! - `scripts-push-json`: `scripts push --json`, with the options of `push --json`;
//! - `save-guards`: the server applies the save guards of each chat, repository and place, holds
//!   saves for review (`held list|save|skip`), and saves the user's own notes and scripts through
//!   `notes save|update` and `scripts save`, which agents cannot reach through MCP.
//!
//! An older server does not know the command and exits non-zero. [`MemoryFeatures::support`] asks
//! once per binary through a [`LineProbe`] (cached per binary, an older server cached as one
//! without features, a failed probe not cached).

use super::memory_line_probe::{LineAnswer, LineProbe};
use crate::env::Env;
use crate::process::ProcessRunner;

/// The line for routing a note scope `remote+local`.
const NOTES_REMOTE_LOCAL: &str = "notes-remote-local";

/// The line for hiding team notes on this computer and discarding local copies.
const NOTES_HIDE: &str = "notes-hide";

/// The line for `push --json`.
const PUSH_JSON: &str = "push-json";

/// The line for routing scripts `remote+local`.
const SCRIPTS_REMOTE_LOCAL: &str = "scripts-remote-local";

/// The line for hiding team scripts on this computer and discarding local copies.
const SCRIPTS_HIDE: &str = "scripts-hide";

/// The line for `scripts push --json`.
const SCRIPTS_PUSH_JSON: &str = "scripts-push-json";

/// The line for save guards, held saves and the user's own saves.
const SAVE_GUARDS: &str = "save-guards";

/// What the installed server says it can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeaturesSupport {
    /// It answers `features`: which of the lines the desktop reads it listed.
    Listed {
        /// `notes-remote-local` is listed.
        notes_remote_local: bool,
        /// `notes-hide` is listed.
        notes_hide: bool,
        /// `push-json` is listed.
        push_json: bool,
        /// `scripts-remote-local` is listed.
        scripts_remote_local: bool,
        /// `scripts-hide` is listed.
        scripts_hide: bool,
        /// `scripts-push-json` is listed.
        scripts_push_json: bool,
        /// `save-guards` is listed.
        save_guards: bool,
    },
    /// It predates `features` (or failed it): none of them.
    Unsupported,
    /// Harness Memory is not installed.
    NotInstalled,
    /// The server could not be asked (it did not start or answer in time).
    Unknown,
}

impl FeaturesSupport {
    /// Whether a note scope may be routed `remote+local`, with hiding team notes and copying local
    /// notes to the team database: all three features are listed.
    pub fn team_and_local(self) -> bool {
        matches!(
            self,
            Self::Listed {
                notes_remote_local: true,
                notes_hide: true,
                push_json: true,
                ..
            }
        )
    }

    /// Whether scripts may be routed `remote+local`, with hiding team scripts and copying local
    /// scripts to the team database: the three script features are listed.
    pub fn scripts_team_and_local(self) -> bool {
        matches!(
            self,
            Self::Listed {
                scripts_remote_local: true,
                scripts_hide: true,
                scripts_push_json: true,
                ..
            }
        )
    }

    /// Whether the server applies save guards, holds saves for review and takes the user's own
    /// saves through its CLI: `save-guards` is listed.
    pub fn save_guards(self) -> bool {
        matches!(self, Self::Listed { save_guards: true, .. })
    }
}

/// The probe of the installed server's features. See the module docs.
#[derive(Debug)]
pub struct MemoryFeatures {
    probe: LineProbe,
}

impl Default for MemoryFeatures {
    fn default() -> Self {
        Self {
            probe: LineProbe::new("features"),
        }
    }
}

impl MemoryFeatures {
    /// No probe yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// What the installed server can do (see the module docs).
    pub async fn support(&self, env: &Env, runner: &dyn ProcessRunner) -> FeaturesSupport {
        match self.probe.ask(env, runner).await {
            answer @ LineAnswer::Listed(_) => FeaturesSupport::Listed {
                notes_remote_local: answer.lists(NOTES_REMOTE_LOCAL),
                notes_hide: answer.lists(NOTES_HIDE),
                push_json: answer.lists(PUSH_JSON),
                scripts_remote_local: answer.lists(SCRIPTS_REMOTE_LOCAL),
                scripts_hide: answer.lists(SCRIPTS_HIDE),
                scripts_push_json: answer.lists(SCRIPTS_PUSH_JSON),
                save_guards: answer.lists(SAVE_GUARDS),
            },
            LineAnswer::Unsupported => FeaturesSupport::Unsupported,
            LineAnswer::NotInstalled => FeaturesSupport::NotInstalled,
            LineAnswer::Unknown => FeaturesSupport::Unknown,
        }
    }
}
