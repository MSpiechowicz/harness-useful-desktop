//! The "Project knowledge" switch (`knowledge_set_enabled`) and what the installed Harness
//! Knowledge can do (`knowledge_status`): the switch stores the `knowledgeEnabled` pref and applies
//! it to the harnesses' user MCP files right away (see `knowledge_setup`).

use harness_business::ecosystem::knowledge_server_setup;
use harness_business::model::{KnowledgeStatus, KnowledgeSwitchResult};
use tauri::State;
use tauri::async_runtime::block_on;

use super::blocking;
use crate::error::CommandResult;
use crate::product::Knowledge;
use crate::state::AppState;
use crate::{sidecar_embedder, sidecar_setup};

/// Whether Harness Knowledge is installed and which features its `features` lists (`features` is
/// all false unless `availability` is `available`). Everything the app offers for it waits for the
/// matching feature.
#[tauri::command]
pub async fn knowledge_status(state: State<'_, AppState>) -> CommandResult<KnowledgeStatus> {
    Ok(state
        .knowledge_features
        .support(&state.env, &state.runner)
        .await
        .status())
}

/// Turns project knowledge on (registers the installed server in every detected harness, also
/// where the user removed it before) or off (removes the app's own entries). Returns the stored
/// pref, the server's install state (while it is not installed nothing is registered, and the UI
/// offers its installer) and, when switched off, the entries left in files with comments for the
/// user to remove by hand. Any other failure to change an MCP file is logged, and the pref is
/// stored regardless. Vector search runs only while project knowledge is on.
#[tauri::command]
pub async fn knowledge_set_enabled(state: State<'_, AppState>, enabled: bool) -> CommandResult<KnowledgeSwitchResult> {
    set_enabled(&state, enabled).await
}

pub(crate) async fn set_enabled(state: &AppState, enabled: bool) -> CommandResult<KnowledgeSwitchResult> {
    let (saved, manual_removals) = sidecar_setup::set_enabled::<Knowledge>(state, enabled).await?;

    sidecar_embedder::reconcile::<Knowledge>(state).await;

    let env = state.env.clone();
    let runner = state.runner;
    let server = blocking(move || Ok(block_on(knowledge_server_setup(&env, &runner)))).await?;

    Ok(KnowledgeSwitchResult {
        knowledge_enabled: saved.knowledge_enabled,
        server,
        manual_removals,
    })
}

#[cfg(test)]
mod tests {
    use harness_business::model::{HarnessId, KnowledgeAvailability};
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{TempDir, cache_detection, home_state, omp_mcp_file};

    fn state(dir: &TempDir) -> AppState {
        let state = home_state(dir);
        cache_detection(&state, HarnessId::Omp, Some(&dir.path().join("missing-omp")));
        for harness in [HarnessId::Pi, HarnessId::OpenCode, HarnessId::Claude, HarnessId::Codex] {
            cache_detection(&state, harness, None);
        }

        state
    }

    #[test]
    fn the_switch_stores_its_own_pref_and_offers_the_installer_while_the_server_is_missing() {
        let dir = TempDir::new("knowledge-switch");
        let state = state(&dir);

        let off = block_on(set_enabled(&state, false)).expect("switched off");
        assert!(!off.knowledge_enabled);
        assert!(!off.server.installed);
        assert!(
            off.server.command.contains("harness-useful-knowledge"),
            "{}",
            off.server.command
        );
        assert_eq!(off.manual_removals, []);

        let prefs = state.prefs.with(|store| store.load());
        assert!(!prefs.knowledge_enabled);
        assert!(prefs.memory_enabled, "Project memory is its own switch");

        let on = block_on(set_enabled(&state, true)).expect("switched on");
        assert!(on.knowledge_enabled);
        assert!(!omp_mcp_file(&state).exists(), "nothing to register without the server");
    }

    #[test]
    fn the_status_says_when_the_server_is_not_installed() {
        let dir = TempDir::new("knowledge-status");
        let state = state(&dir);

        let support = block_on(state.knowledge_features.support(&state.env, &state.runner));

        assert_eq!(support.status().availability, KnowledgeAvailability::NotInstalled);
        assert!(!support.status().features.docs);
    }
}
