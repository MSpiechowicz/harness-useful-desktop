//! The projects the user trusted once in this app run, for harnesses that start only in a folder
//! they trust (`capabilities.projectTrustRequired`, Claude Code). Once the user picks "Trust once"
//! for such a project, the chats the user opens there later in the same run (restarts, approval
//! changes, reopened chats, harness moves, the AI-naming helper) trust it once too, so none of them
//! is refused or loses the chat. Nothing is written anywhere: a fresh app start asks again.
//! Routine runs never use it, their `trustOnce` stays their own. Other harnesses (PI) are left
//! alone.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use crate::harness::Harness;
use crate::model::{AgentSessionInfo, AgentStartRequest, HarnessId};

/// The projects trusted once in this app run, by harness.
#[derive(Debug, Default)]
pub struct TrustMemory {
    trusted: Mutex<HashSet<(HarnessId, PathBuf)>>,
}

impl TrustMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets `request.trust_once` when the user trusted its project once in this app run.
    pub fn apply(&self, harness: &dyn Harness, request: &mut AgentStartRequest) {
        if request.trust_once || !remembers(harness) {
            return;
        }

        let key = (harness.id(), canonical(Path::new(&request.project)));

        if self.lock().contains(&key) {
            request.trust_once = true;
        }
    }

    /// Remembers the project of `info` when its chat trusted it once.
    pub fn remember(&self, harness: &dyn Harness, info: &AgentSessionInfo) {
        if info.trusted_once && remembers(harness) {
            self.lock().insert((harness.id(), canonical(Path::new(&info.project))));
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<(HarnessId, PathBuf)>> {
        self.trusted.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn remembers(harness: &dyn Harness) -> bool {
    let capabilities = harness.capabilities();

    capabilities.project_trust && capabilities.project_trust_required
}

/// The canonical form of `project`, else the path as given (a later start refuses it anyway).
fn canonical(project: &Path) -> PathBuf {
    dunce::canonicalize(project).unwrap_or_else(|_| project.to_path_buf())
}
