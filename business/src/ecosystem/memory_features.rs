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
//! - `save-opt-in`: `guards/global.json` is version 2, saving is off until the user turns it on
//!   (`everyProject`, `onRepos`). A server without it refuses every save against a version 2 file, and a
//!   version 1 file it reads as the old deny list (saving on);
//! - `embedder`: the local embedder (`embedder run|enable|disable|status|remove-model`) behind
//!   vector search;
//! - `settings`: `settings get|set --json` (the English notes setting);
//! - `notes-lang`: `notes list --json` rows carry `lang` and `--lang` filters by it;
//! - `notes-useful`: `notes list --json` rows carry `shown`, `useful` and `lastUsefulAt`, and `--sort
//!   useful|lastUseful` and `--useful ever|never` use them;
//! - `notes-authors`: notes and scripts carry `addedBy` and `updatedBy` (the git name of who wrote
//!   them);
//! - `notes-author-filter`: `notes list` and `scripts list` take `--author NAME` and
//!   `--author-unknown`, sort by `--sort addedBy` and answer the `authors` facet (`notes-authors`
//!   alone only means the rows carry names);
//! - `recall`: `recall --hook`, the notes added to a prompt ([`MemoryFeatures::lists_recall`]);
//! - `offline-update-queue`: a change to a team note made while the team database is unreachable is
//!   kept on this computer (`pending-updates.json`) and applied when it is back;
//! - `note-expect-revision`: `notes update <id> --expect-revision N` refuses a note whose revision
//!   is not `N` (`{"error": "changed"}`);
//! - `sync-command`: `sync --json` copies what waits for the team database now and says what still
//!   waits;
//! - `offline-fallback`: while the team database cannot be opened, notes and scripts of a scope set
//!   to `remote` are saved on this computer, and the config key `offline_fallback` (on by default)
//!   turns that off;
//! - `push-remote-local`: `push --remote-local` and `scripts push --remote-local` copy the entries
//!   of the scopes set to `remote+local`, and only those, to the team database;
//! - `push-selection`: `push --selection -` (and `scripts push --selection -`) takes the local ids
//!   to copy and to overwrite as JSON on stdin and replaces `--overwrite`, so a copy takes only
//!   what the user reviewed;
//! - `pull-json`: `pull --json [--dry-run] [--ids -]` copies the team's notes of the scopes set to
//!   `remote+local` to this computer;
//! - `scripts-pull-json`: `scripts pull`, with the options of `pull`;
//! - `save-place`: `notes save` and `scripts save` take `--place remote|local` for a scope routed
//!   `remote+local` and answer where the entry went (`"place"`);
//! - `remote-local-saves`: the config key `remote_local_saves` (`remote`, or `local`, absent means
//!   remote) decides where agents save new notes and scripts of a scope routed `remote+local`;
//! - `held-save-place`: `held save` takes `--place remote|local` for a held `note_save` or
//!   `script_save` (a held `note_update` refuses it).
//!
//! An older server does not know the command and exits non-zero. [`MemoryFeatures::support`] asks
//! once per binary through a [`LineProbe`] (cached per binary, an older server cached as one
//! without features, a failed probe not cached).

use super::catalog::MEMORY_SIDECAR;
use super::sidecar_line_probe::{LineAnswer, LineProbe};
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

/// The line for opt-in saving: `guards/global.json` version 2.
const SAVE_OPT_IN: &str = "save-opt-in";

/// The line for the local embedder behind vector search.
const EMBEDDER: &str = "embedder";

/// The line for `settings get|set`.
const SETTINGS: &str = "settings";

/// The line for the `lang` of a note and the `--lang` filter.
const NOTES_LANG: &str = "notes-lang";

/// The line for the usefulness counts of a note, `--sort useful|lastUseful` and `--useful`.
const NOTES_USEFUL: &str = "notes-useful";

/// The line for the authors of notes and scripts.
const NOTES_AUTHORS: &str = "notes-authors";

/// The line for the author filter, the `addedBy` sort and the authors facet of the lists.
const NOTES_AUTHOR_FILTER: &str = "notes-author-filter";

/// The line for `recall --hook`, the notes added to a prompt.
const RECALL: &str = "recall";

/// The line for keeping changes to team notes while the team database is unreachable.
const OFFLINE_UPDATE_QUEUE: &str = "offline-update-queue";

/// The line for `notes update --expect-revision`.
const NOTE_EXPECT_REVISION: &str = "note-expect-revision";

/// The line for `sync --json`.
const SYNC_COMMAND: &str = "sync-command";

/// The line for the offline fallback and its `offline_fallback` config key.
const OFFLINE_FALLBACK: &str = "offline-fallback";

/// The line for `push --remote-local` and `scripts push --remote-local`.
const PUSH_REMOTE_LOCAL: &str = "push-remote-local";

/// The line for `push --selection -` and `scripts push --selection -`.
const PUSH_SELECTION: &str = "push-selection";

/// The line for `pull --json`.
const PULL_JSON: &str = "pull-json";

/// The line for `scripts pull --json`.
const SCRIPTS_PULL_JSON: &str = "scripts-pull-json";

/// The line for `notes save --place` and `scripts save --place`.
const SAVE_PLACE: &str = "save-place";

/// The line for the config key `remote_local_saves`.
const REMOTE_LOCAL_SAVES: &str = "remote-local-saves";

/// The line for `held save --place`.
const HELD_SAVE_PLACE: &str = "held-save-place";

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
        /// `save-opt-in` is listed.
        save_opt_in: bool,
        /// `embedder` is listed.
        embedder: bool,
        /// `settings` is listed.
        settings: bool,
        /// `notes-lang` is listed.
        notes_lang: bool,
        /// `notes-useful` is listed.
        notes_useful: bool,
        /// `notes-authors` is listed.
        notes_authors: bool,
        /// `notes-author-filter` is listed.
        notes_author_filter: bool,
        /// `offline-update-queue` is listed.
        offline_update_queue: bool,
        /// `note-expect-revision` is listed.
        note_expect_revision: bool,
        /// `sync-command` is listed.
        sync_command: bool,
        /// `offline-fallback` is listed.
        offline_fallback: bool,
        /// `push-remote-local` is listed.
        push_remote_local: bool,
        /// `push-selection` is listed.
        push_selection: bool,
        /// `pull-json` is listed.
        pull_json: bool,
        /// `scripts-pull-json` is listed.
        scripts_pull_json: bool,
        /// `save-place` is listed.
        save_place: bool,
        /// `remote-local-saves` is listed.
        remote_local_saves: bool,
        /// `held-save-place` is listed.
        held_save_place: bool,
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

    /// Whether the server reads `guards/global.json` version 2 (saving is off until the user turns
    /// it on): `save-opt-in` is listed.
    pub fn save_opt_in(self) -> bool {
        matches!(self, Self::Listed { save_opt_in: true, .. })
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

    /// Whether `notes list` and `scripts list` filter by `--author`, sort by `addedBy` and answer
    /// the authors facet: `notes-author-filter` is listed.
    pub fn notes_author_filter(self) -> bool {
        matches!(
            self,
            Self::Listed {
                notes_author_filter: true,
                ..
            }
        )
    }

    /// Whether a change to a team note made while the team database is unreachable is kept on this
    /// computer and applied later: `offline-update-queue` is listed.
    pub fn offline_update_queue(self) -> bool {
        matches!(
            self,
            Self::Listed {
                offline_update_queue: true,
                ..
            }
        )
    }

    /// Whether `notes update` takes `--expect-revision`: `note-expect-revision` is listed.
    pub fn note_expect_revision(self) -> bool {
        matches!(
            self,
            Self::Listed {
                note_expect_revision: true,
                ..
            }
        )
    }

    /// Whether the server has `sync --json`: `sync-command` is listed.
    pub fn sync_command(self) -> bool {
        matches!(self, Self::Listed { sync_command: true, .. })
    }

    /// Whether the config key `offline_fallback` is known: `offline-fallback` is listed.
    pub fn offline_fallback(self) -> bool {
        matches!(
            self,
            Self::Listed {
                offline_fallback: true,
                ..
            }
        )
    }

    /// Whether `push --remote-local` and `scripts push --remote-local` exist:
    /// `push-remote-local` is listed.
    pub fn push_remote_local(self) -> bool {
        matches!(
            self,
            Self::Listed {
                push_remote_local: true,
                ..
            }
        )
    }

    /// Whether `push --selection -` and `scripts push --selection -` exist: `push-selection` is
    /// listed.
    pub fn push_selection(self) -> bool {
        matches!(
            self,
            Self::Listed {
                push_selection: true,
                ..
            }
        )
    }

    /// Whether a copy to the team database limited to the entries the user reviewed exists:
    /// `push-remote-local` and `push-selection` are both listed.
    pub fn copy_to_remote(self) -> bool {
        self.push_remote_local() && self.push_selection()
    }

    /// Whether `pull --json` exists: `pull-json` is listed.
    pub fn pull_json(self) -> bool {
        matches!(self, Self::Listed { pull_json: true, .. })
    }

    /// Whether `scripts pull --json` exists: `scripts-pull-json` is listed.
    pub fn scripts_pull_json(self) -> bool {
        matches!(
            self,
            Self::Listed {
                scripts_pull_json: true,
                ..
            }
        )
    }

    /// Whether `notes save` and `scripts save` take `--place`: `save-place` is listed.
    pub fn save_place(self) -> bool {
        matches!(self, Self::Listed { save_place: true, .. })
    }

    /// Whether the config key `remote_local_saves` is known (where agents save new entries of a
    /// scope routed to "Remote + this computer"): `remote-local-saves` is listed.
    pub fn remote_local_saves(self) -> bool {
        matches!(
            self,
            Self::Listed {
                remote_local_saves: true,
                ..
            }
        )
    }

    /// Whether `held save` takes `--place` for a held `note_save` or `script_save`:
    /// `held-save-place` is listed.
    pub fn held_save_place(self) -> bool {
        matches!(
            self,
            Self::Listed {
                held_save_place: true,
                ..
            }
        )
    }

    /// Whether notes and scripts carry `addedBy` and `updatedBy`: `notes-authors` is listed.
    pub fn notes_authors(self) -> bool {
        matches!(
            self,
            Self::Listed {
                notes_authors: true,
                ..
            }
        )
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
            probe: LineProbe::new(&MEMORY_SIDECAR, "features"),
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
                save_opt_in: answer.lists(SAVE_OPT_IN),
                embedder: answer.lists(EMBEDDER),
                settings: answer.lists(SETTINGS),
                notes_lang: answer.lists(NOTES_LANG),
                notes_useful: answer.lists(NOTES_USEFUL),
                notes_authors: answer.lists(NOTES_AUTHORS),
                notes_author_filter: answer.lists(NOTES_AUTHOR_FILTER),
                offline_update_queue: answer.lists(OFFLINE_UPDATE_QUEUE),
                note_expect_revision: answer.lists(NOTE_EXPECT_REVISION),
                sync_command: answer.lists(SYNC_COMMAND),
                offline_fallback: answer.lists(OFFLINE_FALLBACK),
                push_remote_local: answer.lists(PUSH_REMOTE_LOCAL),
                push_selection: answer.lists(PUSH_SELECTION),
                pull_json: answer.lists(PULL_JSON),
                scripts_pull_json: answer.lists(SCRIPTS_PULL_JSON),
                save_place: answer.lists(SAVE_PLACE),
                remote_local_saves: answer.lists(REMOTE_LOCAL_SAVES),
                held_save_place: answer.lists(HELD_SAVE_PLACE),
            },
            LineAnswer::Unsupported => FeaturesSupport::Unsupported,
            LineAnswer::NotInstalled => FeaturesSupport::NotInstalled,
            LineAnswer::Unknown => FeaturesSupport::Unknown,
        }
    }

    /// Whether the installed server lists `recall` (`recall --hook`): false when it is not
    /// installed, predates `features` or could not be asked.
    pub(crate) async fn lists_recall(&self, env: &Env, runner: &dyn ProcessRunner) -> bool {
        self.probe.ask(env, runner).await.lists(RECALL)
    }
}
