//! Native file and folder pickers and the save dialog, opened from Rust over the main window.
//!
//! The dialog plugin's JS API is not granted to the webview: every path that reaches the app this
//! way was chosen by the user in an OS dialog. The dialogs do not block the async runtime: the
//! plugin runs the dialog on the main thread and reports through a callback.

use std::path::{Path, PathBuf};

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

/// The file types a file dialog offers: a label and the extensions (without the dot).
#[derive(Debug, Clone, Copy)]
pub struct FileFilter<'a> {
    /// The label of the file type.
    pub name: &'a str,
    /// Its extensions.
    pub extensions: &'a [&'a str],
}

/// The callback the plugin calls with the chosen path (`None` when cancelled).
type Report = Box<dyn FnOnce(Option<FilePath>) + Send + 'static>;

/// Shows a native picker over the main window, opened in `start` when given (otherwise some desktops
/// open on their "Recent" view, a list of unrelated history); `None` when the user cancelled.
pub async fn pick<R: Runtime>(
    app: &AppHandle<R>,
    guard: &DialogGuard,
    kind: Pick,
    title: &str,
    start: Option<&Path>,
) -> CommandResult<Option<PathBuf>> {
    let dialog = file_dialog(app, title, start);

    show(guard, |report| match kind {
        Pick::Folder => dialog.pick_folder(report),
        Pick::File => dialog.pick_file(report),
    })
    .await
}

/// Shows a native file picker limited to `filter`, otherwise like [`pick`].
pub async fn pick_file<R: Runtime>(
    app: &AppHandle<R>,
    guard: &DialogGuard,
    title: &str,
    start: Option<&Path>,
    filter: FileFilter<'_>,
) -> CommandResult<Option<PathBuf>> {
    let dialog = file_dialog(app, title, start).add_filter(filter.name, filter.extensions);

    show(guard, |report| dialog.pick_file(report)).await
}

/// Shows a native save dialog over the main window, opened in `start` with `default_name` filled
/// in; the path to write, or `None` when the user cancelled. The dialog itself asks before
/// replacing an existing file.
pub async fn save<R: Runtime>(
    app: &AppHandle<R>,
    guard: &DialogGuard,
    title: &str,
    start: Option<&Path>,
    default_name: &str,
    filter: FileFilter<'_>,
) -> CommandResult<Option<PathBuf>> {
    let dialog = file_dialog(app, title, start)
        .add_filter(filter.name, filter.extensions)
        .set_file_name(default_name);

    show(guard, |report| dialog.save_file(report)).await
}

/// Opens one dialog through `display` (unless one is already open) and waits for its answer.
async fn show(guard: &DialogGuard, display: impl FnOnce(Report)) -> CommandResult<Option<PathBuf>> {
    let _open = open(guard)?;
    let (sender, mut receiver) = channel(1);
    let report: Report = Box::new(move |picked| {
        sender.try_send(picked).ok();
    });

    display(report);

    picked_path(receiver.recv().await)
}

/// Refuses to stack a second dialog over an open one.
fn open(guard: &DialogGuard) -> CommandResult<OpenDialog<'_>> {
    guard
        .try_open()
        .ok_or_else(|| BusinessError::InvalidInput("a dialog is already open".to_owned()).into())
}

fn file_dialog<R: Runtime>(app: &AppHandle<R>, title: &str, start: Option<&Path>) -> FileDialogBuilder<R> {
    let mut builder = app.dialog().file().set_title(title);

    if let Some(start) = start.filter(|dir| dir.is_dir()) {
        builder = builder.set_directory(start);
    }

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
