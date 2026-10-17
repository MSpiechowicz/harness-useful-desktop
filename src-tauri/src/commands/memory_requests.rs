//! Script run requests of Harness Memory (`memory_script_requests_listen`,
//! `memory_script_decide`): an agent asked to run a script this machine has not approved, and the
//! server waits for the user's answer (see [`crate::memory_requests`]).

use std::sync::Arc;

use harness_business::model::ScriptRequestEvent;
use tauri::State;
use tauri::ipc::Channel;

use super::blocking;
use crate::error::CommandResult;
use crate::memory_requests::ScriptRequests;
use crate::state::AppState;

/// Sends run requests to `on_request`, replacing the previous listener; every request still
/// waiting is sent at once.
#[tauri::command]
pub async fn memory_script_requests_listen(
    state: State<'_, AppState>,
    on_request: Channel<ScriptRequestEvent>,
) -> CommandResult<()> {
    state.script_requests.listen(on_request);

    Ok(())
}

/// Answers run request `request_id`: `allow` runs the script once and approves this version,
/// otherwise the agent is told the user declined. Only a request this app has shown (sent to the
/// listener) and not answered yet can be answered. `invalidInput` for a malformed id, `notFound`
/// for any other id, or when the request is gone (the server stopped waiting).
#[tauri::command]
pub async fn memory_script_decide(state: State<'_, AppState>, request_id: String, allow: bool) -> CommandResult<()> {
    answer(state.script_requests.clone(), request_id, allow).await
}

async fn answer(requests: Arc<ScriptRequests>, request_id: String, allow: bool) -> CommandResult<()> {
    blocking(move || {
        requests.decide(&request_id, allow)?;
        tracing::info!(allow, "memory run request answered");

        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use harness_business::model::IpcErrorCode;
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{REQUEST_ID, memory_data_setup, place_companion, write_script_request};

    #[test]
    fn only_a_shown_request_that_is_not_answered_yet_can_be_answered() {
        let (_dir, env, data) = memory_data_setup("decide");
        place_companion(&env.home, "harness-memory");
        std::fs::create_dir_all(&data).unwrap();
        write_script_request(&data, REQUEST_ID);
        let decision = data.join("requests").join(format!("{REQUEST_ID}.decision"));

        let requests = Arc::new(ScriptRequests::default());
        let answer = |id: &str, allow: bool| block_on(answer(requests.clone(), id.to_owned(), allow));

        let invalid = answer("../x", true).expect_err("invalid id");
        assert_eq!(invalid.code(), IpcErrorCode::InvalidInput);

        // The file is there, but the app has not shown it to the user.
        let unseen = answer(REQUEST_ID, true).expect_err("not shown yet");
        assert_eq!(unseen.code(), IpcErrorCode::NotFound);
        assert!(!decision.exists());

        requests.tick(&env, true, SystemTime::now());
        answer(REQUEST_ID, false).expect("answered");
        assert_eq!(std::fs::read_to_string(&decision).unwrap(), "deny");

        let again = answer(REQUEST_ID, true).expect_err("answered once");
        assert_eq!(again.code(), IpcErrorCode::NotFound);
        assert_eq!(std::fs::read_to_string(&decision).unwrap(), "deny", "the answer stands");
    }
}
