//! Vector search and the English notes setting (`memory_set_vector_search`,
//! `memory_embedder_watch`, `memory_embedder_remove_model`, `memory_settings_get`,
//! `memory_settings_set`), through the installed `harness-memory embedder …` and `settings …`.
//! Each needs the feature of the installed server: `notFound` when Harness Memory is not
//! installed, `unsupported` when it predates it. See `harness_business::ecosystem::memory_embedder`.

use harness_business::Result;
use harness_business::ecosystem::{
    FeaturesSupport, feature_server, get_settings, remove_embedder_model, set_embedder_enabled, set_settings,
};
use harness_business::model::{MemoryEmbedderState, MemorySettings, MemoryVectorSwitchResult};
use tauri::State;
use tauri::ipc::Channel;

use super::blocking;
use crate::error::CommandResult;
use crate::memory_embedder::reconcile;
use crate::state::AppState;

/// Turns vector search on or off: runs `embedder enable|disable`, stores the `vectorSearch` pref,
/// then starts or stops the embedder (it starts only while project memory is on too). The answer
/// holds the stored pref and the embedder's state right then (follow it with
/// `memory_embedder_watch`). Nothing changes when the server refuses: `unsupported` for a Harness
/// Memory without `embedder`, `notFound` when it is not installed, `process` with its line.
#[tauri::command]
pub async fn memory_set_vector_search(
    state: State<'_, AppState>,
    enabled: bool,
) -> CommandResult<MemoryVectorSwitchResult> {
    set_vector_search(&state, enabled).await
}

async fn set_vector_search(state: &AppState, enabled: bool) -> CommandResult<MemoryVectorSwitchResult> {
    let server = embedder_server(state).await?;
    set_embedder_enabled(&state.env, &state.runner, &server, enabled).await?;

    let prefs = state.prefs.clone();
    let saved = blocking(move || Ok(prefs.with(|store| store.update(|prefs| prefs.vector_search = enabled))?)).await?;

    reconcile(state).await;

    Ok(MemoryVectorSwitchResult {
        vector_search: saved.vector_search,
        state: state.memory_embedder.state(),
    })
}

/// Sends the embedder's state (`phase`, `progress`, `model`, `error`) to `on_event` now and on
/// every change, replacing the previous listener.
#[tauri::command]
pub async fn memory_embedder_watch(
    state: State<'_, AppState>,
    on_event: Channel<MemoryEmbedderState>,
) -> CommandResult<()> {
    state.embedder_watch.listen(&state, on_event);

    Ok(())
}

/// Stops the embedder if it runs, then deletes the model files and the vectors
/// (`embedder remove-model`, 60 s). Answers the embedder's state (`off`, `modelPresent` false). The `vectorSearch` pref
/// is left as it is, so turn vector search off first to keep the model from being fetched again at
/// the next launch. `unsupported`, `notFound` and `process` as `memory_set_vector_search`.
#[tauri::command]
pub async fn memory_embedder_remove_model(state: State<'_, AppState>) -> CommandResult<MemoryEmbedderState> {
    let server = embedder_server(&state).await?;

    state.memory_embedder.stop().await;
    remove_embedder_model(&state.env, &state.runner, &server).await?;
    state.memory_embedder.set_model_present(Some(false));

    Ok(state.memory_embedder.state())
}

/// The settings Harness Memory keeps in its store: whether agents must write notes in English (on
/// unless set otherwise). `unsupported` for a Harness Memory without `settings`, `notFound` when it
/// is not installed.
#[tauri::command]
pub async fn memory_settings_get(state: State<'_, AppState>) -> CommandResult<MemorySettings> {
    let server = settings_server(&state).await?;

    Ok(get_settings(&state.env, &state.runner, &server).await?)
}

/// Turns the English notes setting on or off in every store the server writes (a team database
/// applies to the whole team) and answers the setting as stored. Errors as `memory_settings_get`,
/// `process` with the server's line when it refuses.
#[tauri::command]
pub async fn memory_settings_set(state: State<'_, AppState>, english_notes: bool) -> CommandResult<MemorySettings> {
    let server = settings_server(&state).await?;

    Ok(set_settings(&state.env, &state.runner, &server, english_notes).await?)
}

async fn embedder_server(state: &AppState) -> Result<std::path::PathBuf> {
    feature_server(
        &state.env,
        &state.runner,
        &state.memory_features,
        FeaturesSupport::embedder,
        "vector search",
    )
    .await
}

async fn settings_server(state: &AppState) -> Result<std::path::PathBuf> {
    feature_server(
        &state.env,
        &state.runner,
        &state.memory_features,
        FeaturesSupport::settings,
        "memory settings",
    )
    .await
}

#[cfg(test)]
mod tests {
    use harness_business::BusinessError;
    use harness_business::model::{EmbedderPhase, IpcErrorCode};
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{TempDir, home_state, place_memory_server};

    #[test]
    fn nothing_changes_while_harness_memory_is_not_installed() {
        let dir = TempDir::new("vector-not-installed");
        let state = home_state(&dir);

        let switched = block_on(set_vector_search(&state, false)).expect_err("not installed");
        assert_eq!(switched.code(), IpcErrorCode::NotFound);
        assert!(
            state.prefs.with(|store| store.load().vector_search),
            "the pref stays on"
        );

        let settings = block_on(settings_server(&state)).expect_err("not installed");
        assert!(matches!(settings, BusinessError::NotFound(_)));
    }

    #[test]
    fn switching_off_stores_the_pref_and_switching_on_starts_the_embedder() {
        let dir = TempDir::new("vector-switch");
        let state = home_state(&dir);

        if place_memory_server(&dir.path().join("home")).is_none() {
            return;
        }

        let off = block_on(set_vector_search(&state, false)).expect("switched off");
        assert!(!off.vector_search);
        assert_eq!(off.state.phase, EmbedderPhase::Off);
        assert!(!state.prefs.with(|store| store.load().vector_search));

        let on = block_on(set_vector_search(&state, true)).expect("switched on");
        assert!(on.vector_search);
        assert_ne!(on.state.phase, EmbedderPhase::Off, "starting or ready");

        let off = block_on(set_vector_search(&state, false)).expect("switched off again");
        assert_eq!(off.state.phase, EmbedderPhase::Off);
        block_on(state.memory_embedder.stop());
    }

    #[test]
    fn project_memory_off_keeps_the_embedder_stopped_and_the_settings_pass_through() {
        let dir = TempDir::new("vector-memory-off");
        let state = home_state(&dir);

        if place_memory_server(&dir.path().join("home")).is_none() {
            return;
        }

        state
            .prefs
            .with(|store| store.update(|prefs| prefs.memory_enabled = false))
            .expect("memory off");

        let on = block_on(set_vector_search(&state, true)).expect("pref stored");
        assert!(on.vector_search);
        assert_eq!(on.state.phase, EmbedderPhase::Off, "project memory is off");

        let server = block_on(settings_server(&state)).expect("settings are listed");
        let settings = block_on(get_settings(&state.env, &state.runner, &server)).expect("read");
        assert!(settings.english_notes);
    }
}
