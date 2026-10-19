//! Routine runs in the Routines view: "Run now" (`routine_run_now`), "Stop"
//! (`routine_run_stop`), the run history (`routine_runs`) and the next runs of a schedule while it
//! is edited (`routine_schedule_preview`).

use harness_business::model::{RoutineRun, RoutineRunsPage, RoutineRunsQuery, RoutineWhen, RunStatus, SchedulePreview};
use harness_business::routines::{local_zone, schedule_preview};
use jiff::Timestamp;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Runs routine `id` now (valid and approved, on or off). Answers the run when it started, `null`
/// when it waits for a free place (it starts when one frees up). `conflict` while it waits or
/// runs, `invalidInput` for a pending or invalid routine, `notFound` for a missing one.
#[tauri::command]
pub async fn routine_run_now(state: State<'_, AppState>, id: String) -> CommandResult<Option<RoutineRun>> {
    Ok(state.routines.run_now(&id).await?)
}

/// Stops the live run `run_id`; its record ends as `stopped`. `notFound` when it is not running.
#[tauri::command]
pub async fn routine_run_stop(state: State<'_, AppState>, run_id: String) -> CommandResult<()> {
    Ok(state.routines.stop_run(&run_id)?)
}

/// One page of runs, newest first: of one routine or all, optionally with one status.
#[tauri::command]
pub async fn routine_runs(state: State<'_, AppState>, query: RoutineRunsQuery) -> CommandResult<RoutineRunsPage> {
    runs(&state, query).await
}

/// The next runs of `when` in the machine's time zone, or why it is not valid.
#[tauri::command]
pub async fn routine_schedule_preview(when: RoutineWhen) -> CommandResult<SchedulePreview> {
    Ok(schedule_preview(&when, Timestamp::now(), &local_zone()))
}

/// The runs going on come from memory (every one of them is live), the others from the records.
pub(crate) async fn runs(state: &AppState, query: RoutineRunsQuery) -> CommandResult<RoutineRunsPage> {
    if query.status == Some(RunStatus::Running) {
        return Ok(state.routines.live_page(&query));
    }

    let mut page = state.routines.read_runs(move |runs| runs.page(&query)).await?;

    page.runs = state.routines.with_needs_you(page.runs);

    Ok(page)
}
