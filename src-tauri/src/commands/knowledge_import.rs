//! Adding documents to Harness Knowledge: the native pickers (`knowledge_pick_files`,
//! `knowledge_pick_folder`), the window's drop (`knowledge_drop_intake`), the streamed import
//! (`knowledge_import`) and its cancel (`knowledge_import_cancel`), through
//! `harness-knowledge docs import --ndjson`. See `harness_business::ecosystem::knowledge_import`.
//!
//! The webview never names a path: a picker or a drop gives the shell the paths, which are checked
//! and kept (`knowledge_intake`), and the import names them by the token the pick answered. One
//! import runs at a time.

use std::path::PathBuf;
use std::sync::Arc;

use harness_business::ecosystem::{IMPORT_EXTENSIONS, ImportManifest, KnowledgeSupport, run_import};
use harness_business::model::{KnowledgeImportEvent, KnowledgeImportOptions, KnowledgeImportResult, KnowledgeIntake};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use super::{blocking, saved_language};
use crate::dialogs::{self, FileFilter};
use crate::error::CommandResult;
use crate::knowledge_embedder::ready_for_import;
use crate::native_text::native_text;
use crate::state::AppState;

/// Lets the user choose documents (Markdown, text, HTML, OpenAPI, AsciiDoc, reStructuredText, PDF)
/// in a native dialog and checks them. `None` when cancelled. The answer lists what will be
/// imported and what was left out (a symbolic link, a file that is gone) with its reason.
#[tauri::command]
pub async fn knowledge_pick_files(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Option<KnowledgeIntake>> {
    let text = native_text(&saved_language(&state).await?);
    let filter = FileFilter {
        name: text.knowledge_files,
        extensions: &IMPORT_EXTENSIONS,
    };

    let picked = dialogs::pick_files(
        &app,
        &state.dialog,
        text.open_knowledge_files,
        Some(&state.env.home),
        filter,
    )
    .await?;

    match picked {
        Some(paths) => Ok(Some(put(&state, paths).await?)),
        None => Ok(None),
    }
}

/// Lets the user choose a folder of documents in a native dialog and checks it. `None` when
/// cancelled. The server walks it at import time: it skips symbolic links, hidden entries, `.git`,
/// `node_modules` and `target`, goes at most 16 levels deep and takes at most 5000 files.
#[tauri::command]
pub async fn knowledge_pick_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Option<KnowledgeIntake>> {
    let text = native_text(&saved_language(&state).await?);

    let picked = dialogs::pick_folder(&app, &state.dialog, text.open_knowledge_folder, Some(&state.env.home)).await?;

    match picked {
        Some(path) => Ok(Some(put(&state, vec![path]).await?)),
        None => Ok(None),
    }
}

/// The files and folders of the last drop on the window, checked, once: `None` when nothing was
/// dropped since. The paths stay in the shell, as with the pickers.
#[tauri::command]
pub async fn knowledge_drop_intake(state: State<'_, AppState>) -> CommandResult<Option<KnowledgeIntake>> {
    let intakes = Arc::clone(&state.knowledge_intakes);

    blocking(move || Ok(intakes.take_drop())).await
}

async fn put(state: &AppState, paths: Vec<PathBuf>) -> CommandResult<KnowledgeIntake> {
    let intakes = Arc::clone(&state.knowledge_intakes);

    blocking(move || Ok(intakes.put(&paths))).await
}

/// Imports the files and folders of the intake `token`, linking every document to
/// `options.links` (none: global) and replacing document `options.replaces` in place when given
/// (one file only). Each event of `harness-knowledge docs import --ndjson` goes to `on_event` as it
/// happens: `plan`, then `file` and `progress` after each file, then `done`; `failed` when the
/// import could not run. The embedder is started first while vector search is on, so the chunks get
/// their vectors. Re-importing unchanged content embeds nothing again. `invalidInput` while another
/// import runs, `notFound` for a token that is gone, `unsupported` for a server without
/// `import-ndjson`. The answer holds the counts of the closing event.
#[tauri::command]
pub async fn knowledge_import(
    state: State<'_, AppState>,
    token: String,
    options: Option<KnowledgeImportOptions>,
    on_event: Channel<KnowledgeImportEvent>,
) -> CommandResult<KnowledgeImportResult> {
    let server = state
        .knowledge_features
        .feature_server(
            &state.env,
            &state.runner,
            KnowledgeSupport::import,
            "importing documents",
        )
        .await?;
    let paths = state.knowledge_intakes.get(&token)?;
    let run = state.knowledge_importer.begin()?;

    ready_for_import(&state).await;

    let manifest = ImportManifest::new(paths.files, paths.folders, options.unwrap_or_default());
    let spawner = state.agents.spawner();

    let result = run_import(&state.env, spawner.as_ref(), &server, &manifest, &run, |event| {
        // A closed channel only means the page went away: the import goes on.
        on_event.send(event).ok();
    })
    .await?;

    state.knowledge_intakes.forget(&token);

    Ok(result)
}

/// Asks the running import to stop between two documents (SIGTERM on Linux and macOS, ending the
/// process on Windows). The files done before it stay, and the import ends with its `done` event
/// (`cancelled`). False when no import runs.
#[tauri::command]
pub async fn knowledge_import_cancel(state: State<'_, AppState>) -> CommandResult<bool> {
    Ok(state.knowledge_importer.cancel())
}
