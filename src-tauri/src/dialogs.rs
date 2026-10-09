//! Native file and folder pickers, opened from Rust over the main window.
//!
//! The dialog plugin's JS API is not granted to the webview: every path that reaches the app this
//! way was chosen by the user in an OS dialog. The pickers do not block the async runtime: the
//! plugin runs the dialog on the main thread and reports through a callback.

use std::path::PathBuf;

use harness_business::BusinessError;
use tauri::async_runtime::channel;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};

use crate::error::{CommandError, CommandResult};
use crate::state::{DialogGuard, OpenDialog};
use crate::window::MAIN_WINDOW;

/// What the user picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// A directory.
    Folder,
    /// A file.
    File,
}

/// Shows a native picker over the main window; `None` when the user cancelled.
pub async fn pick<R: Runtime>(
    app: &AppHandle<R>,
    guard: &DialogGuard,
    kind: Pick,
    title: &str,
) -> CommandResult<Option<PathBuf>> {
    let _open = open(guard)?;
    let (sender, mut receiver) = channel(1);
    let report = move |picked| {
        sender.try_send(picked).ok();
    };

    match kind {
        Pick::Folder => file_dialog(app, title).pick_folder(report),
        Pick::File => file_dialog(app, title).pick_file(report),
    }

    picked_path(receiver.recv().await)
}

/// Refuses to stack a second dialog over an open one.
fn open(guard: &DialogGuard) -> CommandResult<OpenDialog<'_>> {
    guard
        .try_open()
        .ok_or_else(|| BusinessError::InvalidInput("a dialog is already open".to_owned()).into())
}

fn file_dialog<R: Runtime>(app: &AppHandle<R>, title: &str) -> FileDialogBuilder<R> {
    let builder = app.dialog().file().set_title(title);

    match app.get_webview_window(MAIN_WINDOW) {
        Some(window) => builder.set_parent(&window),
        None => builder,
    }
}

/// `received` is `None` when the plugin dropped the callback without calling it (the dialog
/// could not be shown); `Some(None)` means the user cancelled.
fn picked_path(received: Option<Option<FilePath>>) -> CommandResult<Option<PathBuf>> {
    let Some(picked) = received else {
        return Err(CommandError::internal("the dialog could not be shown"));
    };

    let Some(picked) = picked else {
        return Ok(None);
    };

    picked
        .into_path()
        .map(Some)
        .map_err(|_| BusinessError::InvalidInput("the dialog returned an unsupported location".to_owned()).into())
}
