//! The code graph of Harness Code: the setting that registers its server (`code_set_enabled`) and
//! the code index switch and builds (`code_index_get`, `code_index_set`,
//! `code_builds_listen`, `code_prepare`), through the installed `harness-code …`. The switch needs the `code-index`
//! feature of the installed server: `notFound` when Harness Code is not installed, `unsupported`
//! when it predates it. See `harness_business::ecosystem::code`.

use harness_business::Result;
use harness_business::ecosystem::{
    CodeSupport, code_server_setup, code_switch_is_on, get_code_index, prepare_code_index, set_code_index,
};
use harness_business::model::{CodeBuildEvent, CodeIndexState, CodeSwitchResult};
use harness_business::prefs::PrefsStore;
use tauri::State;
use tauri::async_runtime::block_on;
use tauri::ipc::Channel;

use super::blocking;
use crate::error::CommandResult;
use crate::hooks::seed_available;
use crate::product::Code;
use crate::sidecar_setup;
use crate::state::AppState;

/// Turns the code graph on (registers the installed server in every detected harness, also where
/// the user removed it before) or off (removes the app's own entries). Returns the stored pref, the
/// server's install state (while it is not installed nothing is registered, and the UI offers its
/// installer) and, when switched off, the entries left in files with comments for the user to
/// remove by hand. Any other failure to change an MCP file is logged, and the pref is stored
/// regardless. The code index switch (`code_index_set`) is a separate choice of the server.
#[tauri::command]
pub async fn code_set_enabled(state: State<'_, AppState>, enabled: bool) -> CommandResult<CodeSwitchResult> {
    set_enabled(&state, enabled).await
}

pub(crate) async fn set_enabled(state: &AppState, enabled: bool) -> CommandResult<CodeSwitchResult> {
    let (saved, manual_removals) = sidecar_setup::set_enabled::<Code>(state, enabled).await?;

    // The hook that needs the code graph runs or pauses from now on. Switched on, it is seeded when
    // it never was.
    state.hooks.refresh_members();
    if enabled {
        seed_available(&state.hooks).await;
    }

    let env = state.env.clone();
    let runner = state.runner;
    let server = blocking(move || Ok(block_on(code_server_setup(&env, &runner)))).await?;

    Ok(CodeSwitchResult {
        code_enabled: saved.code_enabled,
        server,
        manual_removals,
    })
}

/// Whether the code index is on (`status --json`) and where the switch comes from
/// (`default`, `file` or `env`). `unsupported` for a Harness Code without `code-index`, `notFound`
/// when it is not installed.
#[tauri::command]
pub async fn code_index_get(state: State<'_, AppState>) -> CommandResult<CodeIndexState> {
    let server = code_server(&state).await?;

    Ok(get_code_index(&state.env, &state.runner, &server).await?)
}

/// Turns the code index on or off (`enable|disable --json`; off hides the `code_*` tools and
/// keeps the index) and answers the state as the server reports it. Errors as
/// `code_index_get`, `process` with the server's line when it cannot write the choice.
#[tauri::command]
pub async fn code_index_set(state: State<'_, AppState>, on: bool) -> CommandResult<CodeIndexState> {
    let server = code_server(&state).await?;
    let saved = set_code_index(&state.env, &state.runner, &server, on).await?;

    // "Use the code graph first" runs or pauses from now on, and is seeded when it never was.
    state.hooks.refresh_members();
    if saved.enabled {
        seed_available(&state.hooks).await;
    }

    Ok(saved)
}

/// Sends the code graph builds to `on_event` (one event when a build begins, moves into another
/// 10 % step of its files or ends), replacing the previous listener; the builds still in progress
/// are sent at once. `phase` is `building`, `ready`, `error`, `stale` or `cancelled` (the last two
/// mean: dismiss it).
#[tauri::command]
pub async fn code_builds_listen(state: State<'_, AppState>, on_event: Channel<CodeBuildEvent>) -> CommandResult<()> {
    state.code_builds.listen(on_event);

    Ok(())
}

/// Builds the code index of `project` ahead of the first agent call, so the agent does not wait
/// for it (`build --project P --json`). The UI calls it when a project opens and does not wait
/// for the answer: a build of a huge repository takes minutes, and its progress comes through
/// `code_builds_listen`. It does nothing when the Code graph setting or the code index is off, the installed Harness
/// Code lacks `code-prepare` or is not installed, the project path does not resolve in a few
/// seconds, the project is already waiting for or running its build, too many projects wait, or off
/// Linux and macOS. Builds run one at a time: a project opened while another builds waits its turn. A failed build is logged and never an error, because nobody is waiting to read it.
#[tauri::command]
pub async fn code_prepare(state: State<'_, AppState>, project: String) -> CommandResult<()> {
    // The build on open runs on Linux and macOS. Windows has no worker memory cap, so there the
    // index is built on the first code tool call of an agent.
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        return Ok(());
    }

    if !prepare_allowed(&state).await {
        return Ok(());
    }

    // `/r`, `/r/.` and a symlink to it are one repository: the server is given the real path.
    let Some(project) = state.code_prepares.resolve(project).await else {
        return Ok(());
    };

    let Some(queued) = state.code_prepares.enqueue(project.clone()) else {
        return Ok(());
    };

    let _running = queued.turn().await;

    // The setting or the index switch may have been turned off while this build waited.
    if !prepare_allowed(&state).await {
        return Ok(());
    }

    let Ok(server) = prepare_server(&state).await else {
        return Ok(());
    };

    if let Err(error) = prepare_code_index(&state.env, &state.runner, &server, &project).await {
        tracing::warn!(%error, "code index could not be prepared");
    }

    Ok(())
}

/// Whether a build on open may run: the Code graph setting and the code index switch are both on.
async fn prepare_allowed(state: &AppState) -> bool {
    let env = state.env.clone();
    let prefs = state.prefs.clone();

    blocking(move || Ok(prefs.with(|store| store.load()).code_enabled && code_switch_is_on(&env)))
        .await
        .unwrap_or(false)
}

/// Whether the background updater may install Harness Code on its own, once: true when the Code
/// graph setting and both auto-update switches are on and this was not claimed before, and then the claim is recorded at once (a
/// failed or later removed Harness Code is never installed again unasked). The page asks only
/// while Harness Memory is installed and Harness Code is not, then runs `setup_install_code`.
#[tauri::command]
pub async fn code_auto_install_claim(state: State<'_, AppState>) -> CommandResult<bool> {
    let prefs = state.prefs.clone();

    blocking(move || Ok(prefs.with(claim_auto_install)?)).await
}

fn claim_auto_install(store: &PrefsStore) -> Result<bool> {
    let stored = store.load();

    // The same switches as the page's background updater: nothing installs on its own while either is off.
    let updates = stored.auto_update_extensions && stored.check_companion_updates;

    if !stored.code_enabled || stored.code_auto_install_done || !updates {
        return Ok(false);
    }

    store.update(|prefs| prefs.code_auto_install_done = true)?;

    Ok(true)
}

async fn prepare_server(state: &AppState) -> Result<std::path::PathBuf> {
    state
        .code_features
        .feature_server(
            &state.env,
            &state.runner,
            CodeSupport::prepare,
            "preparing the code index",
        )
        .await
}

async fn code_server(state: &AppState) -> Result<std::path::PathBuf> {
    state
        .code_features
        .feature_server(&state.env, &state.runner, CodeSupport::index, "the code index")
        .await
}

#[cfg(test)]
mod tests {
    use harness_business::model::HarnessId;
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{TempDir, cache_detection, home_state, omp_mcp_file};

    #[test]
    fn the_setting_stores_its_own_pref_and_offers_the_installer_while_the_server_is_missing() {
        let dir = TempDir::new("code-switch");
        let state = home_state(&dir);
        for harness in [
            HarnessId::Omp,
            HarnessId::Pi,
            HarnessId::OpenCode,
            HarnessId::Claude,
            HarnessId::Codex,
        ] {
            cache_detection(&state, harness, None);
        }

        let off = block_on(set_enabled(&state, false)).expect("switched off");
        assert!(!off.code_enabled);
        assert!(!off.server.installed);
        assert!(
            off.server.command.contains("harness-useful-code"),
            "{}",
            off.server.command
        );
        assert_eq!(off.manual_removals, []);

        let prefs = state.prefs.with(|store| store.load());
        assert!(!prefs.code_enabled);
        assert!(
            prefs.memory_enabled && prefs.knowledge_enabled,
            "each product has its own switch"
        );

        let on = block_on(set_enabled(&state, true)).expect("switched on");
        assert!(on.code_enabled);
        assert!(!omp_mcp_file(&state).exists(), "nothing to register without the server");
    }

    #[test]
    fn the_background_install_is_claimed_once_and_never_while_the_setting_is_off() {
        let dir = TempDir::new("code-auto-install");
        let state = home_state(&dir);

        let first = state.prefs.with(claim_auto_install).expect("claimed");
        let second = state.prefs.with(claim_auto_install).expect("asked again");

        assert!(first, "the first ask may install");
        assert!(!second, "a later ask, also after a removal, may not");
        assert!(state.prefs.with(|store| store.load()).code_auto_install_done);

        let off = TempDir::new("code-auto-install-off");
        let state = home_state(&off);
        block_on(set_enabled(&state, false)).expect("switched off");

        assert!(
            !state.prefs.with(claim_auto_install).expect("asked"),
            "the setting is off"
        );
        assert!(!state.prefs.with(|store| store.load()).code_auto_install_done);
    }

    #[test]
    fn the_background_install_is_refused_without_spending_the_claim_while_an_update_switch_is_off() {
        for (extensions, companions) in [(false, true), (true, false), (false, false)] {
            let dir = TempDir::new("code-auto-install-updates");
            let state = home_state(&dir);
            state
                .prefs
                .with(|store| {
                    store.update(|prefs| {
                        prefs.auto_update_extensions = extensions;
                        prefs.check_companion_updates = companions;
                    })
                })
                .expect("stored");

            assert!(
                !state.prefs.with(claim_auto_install).expect("asked"),
                "{extensions} {companions}"
            );
            assert!(!state.prefs.with(|store| store.load()).code_auto_install_done);

            state
                .prefs
                .with(|store| {
                    store.update(|prefs| {
                        prefs.auto_update_extensions = true;
                        prefs.check_companion_updates = true;
                    })
                })
                .expect("stored");

            assert!(
                state.prefs.with(claim_auto_install).expect("asked"),
                "still free once they are on"
            );
        }
    }
}
