//! What the installed Harness Memory can do beyond its commands: `harness-memory features` prints
//! one feature per line and exits 0. The desktop reads these:
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
//!   `notes save|update` and `scripts save`, which agents cannot reach through MCP;
//! - `embedder`: the local embedder (`embedder run|enable|disable|status|remove-model`) behind
//!   vector search;
//! - `settings`: `settings get|set --json` (the English notes setting);
//! - `notes-lang`: `notes list --json` rows carry `lang` and `--lang` filters by it;
//! - `notes-useful`: `notes list --json` rows carry `shown`, `useful` and `lastUsefulAt`, and `--sort
//!   useful|lastUseful` and `--useful ever|never` use them;
//! - `code-index`: `code status|enable|disable --json`, the switch of the code index behind the
//!   `code_*` tools;
//! - `code-prepare`: `code build --project P --json`, the build of a repository's code index before
//!   an agent asks for it.
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

/// The line for the local embedder behind vector search.
const EMBEDDER: &str = "embedder";

/// The line for `settings get|set`.
const SETTINGS: &str = "settings";

/// The line for the `lang` of a note and the `--lang` filter.
const NOTES_LANG: &str = "notes-lang";

/// The line for the usefulness counts of a note, `--sort useful|lastUseful` and `--useful`.
const NOTES_USEFUL: &str = "notes-useful";

/// The line for the code index switch (`code status|enable|disable`).
const CODE_INDEX: &str = "code-index";

/// The line for building the code index of a repository ahead of the first agent call.
const CODE_PREPARE: &str = "code-prepare";

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
        /// `embedder` is listed.
        embedder: bool,
        /// `settings` is listed.
        settings: bool,
        /// `notes-lang` is listed.
        notes_lang: bool,
        /// `notes-useful` is listed.
        notes_useful: bool,
        /// `code-index` is listed.
        code_index: bool,
        /// `code-prepare` is listed.
        code_prepare: bool,
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

    /// Whether the server runs a local embedder for vector search: `embedder` is listed.
    pub fn embedder(self) -> bool {
        matches!(self, Self::Listed { embedder: true, .. })
    }

    /// Whether the server has `settings get|set`: `settings` is listed.
    pub fn settings(self) -> bool {
        matches!(self, Self::Listed { settings: true, .. })
    }

    /// Whether notes carry a `lang` and `notes list` filters by `--lang`: `notes-lang` is listed.
    pub fn notes_lang(self) -> bool {
        matches!(self, Self::Listed { notes_lang: true, .. })
    }

    /// Whether notes carry usefulness counts and `notes list` sorts by `useful|lastUseful` and
    /// filters by `--useful`: `notes-useful` is listed.
    pub fn notes_useful(self) -> bool {
        matches!(self, Self::Listed { notes_useful: true, .. })
    }

    /// Whether the server has `code status|enable|disable`: `code-index` is listed.
    pub fn code_index(self) -> bool {
        matches!(self, Self::Listed { code_index: true, .. })
    }

    /// Whether the server has `code build`: `code-prepare` is listed.
    pub fn code_prepare(self) -> bool {
        matches!(self, Self::Listed { code_prepare: true, .. })
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
                embedder: answer.lists(EMBEDDER),
                settings: answer.lists(SETTINGS),
                notes_lang: answer.lists(NOTES_LANG),
                notes_useful: answer.lists(NOTES_USEFUL),
                code_index: answer.lists(CODE_INDEX),
                code_prepare: answer.lists(CODE_PREPARE),
            },
            LineAnswer::Unsupported => FeaturesSupport::Unsupported,
            LineAnswer::NotInstalled => FeaturesSupport::NotInstalled,
            LineAnswer::Unknown => FeaturesSupport::Unknown,
        }
    }
}
