//! Commands that hand something to the OS: revealing a file in the file manager and opening a
//! link in the default browser. Both are limited by the rules in [`crate::access`]; links also
//! by the `openAnyLink` preference.

use std::path::{Path, PathBuf};

use harness_business::BusinessError;
use harness_business::app_dirs::exports_dir;
use harness_business::prefs::PrefsOrigin;
use tauri::State;

use super::blocking;
use super::projects::active_project;
use crate::access::{allowed_link, any_link, link_allowlist, reveal_target};
use crate::error::CommandResult;
use crate::state::{AppState, SharedPrefs};

/// Opens the OS file manager with `path` selected. The path must exist and lie inside a harness
/// configuration directory (config root or agent dir of any registered harness), the active
/// project or the app's exports folder; anything else is `forbidden`.
#[tauri::command]
pub async fn reveal_path(state: State<'_, AppState>, path: String) -> CommandResult<()> {
    let active = active_project(&state).await?;
    let roots = reveal_roots(&state, active.as_deref());

    blocking(move || {
        let target = reveal_target(&roots, Path::new(&path))?;

        tauri_plugin_opener::reveal_item_in_dir(&target)
            .map_err(|error| BusinessError::Process(format!("cannot open the file manager: {error}")).into())
    })
    .await
}

/// Opens an `https` link in the default browser: an allowlisted one, or, when the user turned on
/// `openAnyLink`, any one plus plain `http` to this computer; anything else is `forbidden`.
#[tauri::command]
pub async fn open_external(state: State<'_, AppState>, url: String) -> CommandResult<()> {
    let url = if any_link_on(&state.prefs).await {
        any_link(&url)?
    } else {
        allowed_link(&link_allowlist(&state.registry), &url)?
    };

    blocking(move || {
        tauri_plugin_opener::open_url(url.as_str(), None::<&str>)
            .map_err(|error| BusinessError::Process(format!("cannot open the browser: {error}")).into())
    })
    .await
}

/// The `openAnyLink` preference. Fails closed: off unless a usable prefs file turns it on.
async fn any_link_on(prefs: &SharedPrefs) -> bool {
    let prefs = prefs.clone();

    let loaded = blocking(move || Ok(prefs.with(|store| store.load_checked()))).await;

    matches!(loaded, Ok((prefs, PrefsOrigin::Stored)) if prefs.open_any_link)
}

/// Directories whose contents may be revealed: each harness's config root and agent dir, the
/// active project, and the app's exports folder (where chat exports are written).
fn reveal_roots(state: &AppState, active: Option<&Path>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();

    for harness in state.registry.all() {
        let paths = harness.paths(&state.env, active);
        roots.push(paths.config_root);
        roots.push(paths.agent_dir);
    }

    roots.extend(active.map(Path::to_path_buf));
    roots.extend(exports_dir(&state.env));

    roots
}

#[cfg(test)]
mod tests {
    use harness_business::env::Env;
    use harness_business::model::AppPrefs;
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::TempDir;

    fn state(dir: &TempDir) -> AppState {
        AppState::new(dir.path().join("config"), Env::for_tests(dir.path().join("home")))
    }

    #[test]
    fn any_link_follows_the_saved_preference() {
        let dir = TempDir::new("links-saved");
        let state = state(&dir);

        for open_any_link in [true, false] {
            state.prefs.with(|store| {
                store
                    .save(&AppPrefs {
                        open_any_link,
                        ..AppPrefs::default()
                    })
                    .expect("save prefs")
            });

            assert_eq!(block_on(any_link_on(&state.prefs)), open_any_link);
        }
    }

    #[test]
    fn a_chat_export_can_be_revealed() {
        let dir = TempDir::new("reveal-export");
        // Windows keeps exports under `%LOCALAPPDATA%`, which test envs leave unset.
        let env = Env::for_tests(dir.path().join("home"))
            .with_var("LOCALAPPDATA", dir.path().join("local").to_string_lossy());
        let state = AppState::new(dir.path().join("config"), env);
        let export = exports_dir(&state.env)
            .expect("exports folder")
            .join("opencode-ses_1.html");
        std::fs::create_dir_all(export.parent().unwrap()).unwrap();
        std::fs::write(&export, "<html></html>").unwrap();

        let target = reveal_target(&reveal_roots(&state, None), &export).expect("an export is revealed");

        assert_eq!(target, export.canonicalize().unwrap());
    }

    #[test]
    fn any_link_is_off_without_prefs_or_with_unusable_ones() {
        let dir = TempDir::new("links-unusable");
        let state = state(&dir);
        assert!(!block_on(any_link_on(&state.prefs)));

        let prefs = state.prefs.with(|store| store.path());
        std::fs::create_dir_all(prefs.parent().unwrap()).unwrap();
        std::fs::write(&prefs, r#"{"version":1,"openAnyLink":true,"theme":"bogus"}"#).unwrap();

        assert!(!block_on(any_link_on(&state.prefs)));
    }
}
