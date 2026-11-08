//! Which copy of notes or scripts a call asks for ([`CopyTarget`]), whether the user allowed it
//! and the installed server can do it ([`CopyGate`]), and the command it runs
//! ([`CopyTarget::args`]), for [`super::memory_notes_copy`].
//!
//! - `stranded` is the copy after a scope moved to the team database: `push` (`scripts push`) with
//!   no scope flag. Neither a switch nor the server's `features` are asked for it.
//! - `toRemote` is `push --remote-local` (`scripts push --remote-local`): the entries of the scopes
//!   set to `remote+local`, and only those. It needs the switch "Copy to remote" and the
//!   `push-remote-local` and `push-selection` features: the copy takes only the entries picked
//!   from the review, so a server that cannot be told which is `unsupported`.
//! - `toLocal` is `pull` (`scripts pull`): the team's entries of those scopes. It needs the switch
//!   "Copy to this computer" and the `pull-json` (`scripts-pull-json`) feature.
//!
//! A switch that is off is `forbidden`, a server without the feature `unsupported` and one that is
//! not installed `notFound`. Each is checked before the copy runs (a
//! feature is read from `harness-memory features`, which may run first).

use std::ffi::OsString;
use std::path::PathBuf;

use super::memory_embedder::feature_server;
use super::memory_features::{FeaturesSupport, MemoryFeatures};
use super::memory_hidden::entry_server;
use super::memory_scripts::MemoryScripts;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{MemoryCopyKind, NotesCopyMode};
use crate::process::ProcessRunner;

/// What a copy takes and which way it goes. Previews are kept per target, so one of another
/// target is never used for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CopyTarget {
    /// Notes or scripts.
    pub kind: MemoryCopyKind,
    /// The direction.
    pub mode: NotesCopyMode,
}

impl From<MemoryCopyKind> for CopyTarget {
    /// The `stranded` copy of `kind`.
    fn from(kind: MemoryCopyKind) -> Self {
        Self {
            kind,
            mode: NotesCopyMode::Stranded,
        }
    }
}

impl CopyTarget {
    /// The command of the dry run and of the copy, as an error quotes them.
    pub(super) fn names(self) -> (&'static str, &'static str) {
        match (self.kind, self.mode) {
            (MemoryCopyKind::Notes, NotesCopyMode::ToLocal) => ("pull --dry-run", "pull"),
            (MemoryCopyKind::Scripts, NotesCopyMode::ToLocal) => ("scripts pull --dry-run", "scripts pull"),
            (MemoryCopyKind::Notes, _) => ("push --dry-run", "push"),
            (MemoryCopyKind::Scripts, _) => ("scripts push --dry-run", "scripts push"),
        }
    }

    /// The command with `flags`: `push` or `pull` (`scripts push`, `scripts pull`), `--remote-local`
    /// for `toRemote` only, then `flags`. Never a `--scope`: a plain push takes what the server
    /// routes `remote`, and `--scope` would opt a `remote+local` scope in and so delete entries
    /// kept on this computer.
    pub(super) fn args(self, flags: &[&str]) -> Vec<OsString> {
        let command: &[&str] = match (self.kind, self.mode) {
            (MemoryCopyKind::Notes, NotesCopyMode::ToLocal) => &["pull"],
            (MemoryCopyKind::Scripts, NotesCopyMode::ToLocal) => &["scripts", "pull"],
            (MemoryCopyKind::Notes, _) => &["push"],
            (MemoryCopyKind::Scripts, _) => &["scripts", "push"],
        };

        let direction: &[&str] = match self.mode {
            NotesCopyMode::ToRemote => &["--remote-local"],
            NotesCopyMode::Stranded | NotesCopyMode::ToLocal => &[],
        };

        command
            .iter()
            .chain(direction)
            .chain(flags)
            .map(OsString::from)
            .collect()
    }
}

/// What the user allowed (the Settings switches "Copy to remote" and "Copy to this computer") and
/// what the installed server lists.
#[derive(Clone, Copy)]
pub struct CopyGate<'a> {
    /// The server's features, asked only for `toRemote` and `toLocal`.
    pub features: &'a MemoryFeatures,
    /// The switch "Copy to remote".
    pub to_remote: bool,
    /// The switch "Copy to this computer".
    pub to_local: bool,
}

impl CopyGate<'_> {
    /// The installed server, when a copy of `target` is allowed and the server can do it (see the
    /// module docs). `stranded` only needs a server that has the notes view or the scripts.
    pub(super) async fn server(
        &self,
        env: &Env,
        probe: &MemoryScripts,
        runner: &dyn ProcessRunner,
        target: CopyTarget,
    ) -> Result<PathBuf> {
        let (allowed, feature, what): (bool, fn(FeaturesSupport) -> bool, &str) = match (target.mode, target.kind) {
            (NotesCopyMode::Stranded, _) => return entry_server(env, probe, target.kind).await,
            (NotesCopyMode::ToRemote, _) => (
                self.to_remote,
                FeaturesSupport::copy_to_remote,
                "copying to the remote database",
            ),
            (NotesCopyMode::ToLocal, MemoryCopyKind::Notes) => (
                self.to_local,
                FeaturesSupport::pull_json,
                "copying notes to this computer",
            ),
            (NotesCopyMode::ToLocal, MemoryCopyKind::Scripts) => (
                self.to_local,
                FeaturesSupport::scripts_pull_json,
                "copying scripts to this computer",
            ),
        };

        if !allowed {
            return Err(BusinessError::Forbidden(format!(
                "{what} is turned off in Settings, so turn it on first"
            )));
        }

        feature_server(env, runner, self.features, feature, what).await
    }
}
