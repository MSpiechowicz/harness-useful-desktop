//! What a running job reports back while it runs: the harness session it got, and whether it
//! waits for an answer (see [`super::job::AgentRunner`]).

use std::sync::Arc;

use harness_business::model::{RoutinesEvent, RunSession, SessionRoutineTag};
use harness_business::routines::SessionTags;

use super::RoutineRuntime;
use super::notify::Alert;

/// What a running job reports back (see [`super::job::AgentRunner`]).
#[derive(Clone)]
pub struct RunProgress {
    pub(super) runtime: Arc<RoutineRuntime>,
    pub(super) routine_id: String,
    pub(super) run_id: String,
    pub(super) name: String,
}

impl RunProgress {
    /// Records the harness session of the run and tags it for Sessions.
    pub async fn session(&self, session: RunSession) {
        let Some(local_data) = self.runtime.local_data.clone() else {
            return;
        };

        self.runtime
            .lock_runs()
            .set_session(&self.routine_id, &self.run_id, session.clone());

        let (routine_id, run_id) = (self.routine_id.clone(), self.run_id.clone());
        let tag = SessionRoutineTag {
            routine_id: routine_id.clone(),
            run_id: run_id.clone(),
            name: self.name.clone(),
        };

        let recorded = self
            .runtime
            .record(move |runs| {
                SessionTags::new(&local_data).record(session.harness, &session.id, &tag)?;
                runs.update(&routine_id, &run_id, |record| record.session = Some(session))
            })
            .await;

        if let Err(error) = recorded {
            tracing::warn!(run = self.run_id, code = ?error.code(), "routine run session not recorded");
        }
    }

    /// Whether the run waits for an answer now. Turning it on tells the Routines view
    /// (`needsYou`) and the user, turning it off tells the view (`needsYouCleared`).
    pub fn needs_you(&self, waiting: bool) {
        let changed = self
            .runtime
            .lock_runs()
            .set_needs_you(&self.routine_id, &self.run_id, waiting);

        if !changed {
            return;
        }

        if !waiting {
            self.runtime.emit(RoutinesEvent::NeedsYouCleared {
                run_id: self.run_id.clone(),
                routine_id: self.routine_id.clone(),
            });
            return;
        }

        self.runtime.emit(RoutinesEvent::NeedsYou {
            run_id: self.run_id.clone(),
            routine_id: self.routine_id.clone(),
            name: self.name.clone(),
        });
        self.runtime
            .alerts
            .send(&Alert::needs_you(&self.routine_id, &self.run_id, &self.name));
    }
}
