//! Files dragged onto the window.
//!
//! The webview's own drag and drop cannot read a file the file manager drags in (it only sees a
//! `file://` text), so the window's native drag and drop is on (`dragDropEnabled`) and its paths
//! stay in Rust: the images among them are read here and sent to the webview over the channel
//! `drop_listen` registered. The webview never names a path.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use harness_business::dropped::read_dropped;
use harness_business::model::DropEvent;
use tauri::ipc::Channel;
use tauri::{DragDropEvent, Manager, Runtime, WebviewWindow, WindowEvent};

use crate::state::AppState;

/// Where drag and drop events go: the channel of the webview that last called `drop_listen`.
#[derive(Default)]
pub struct DropSink(Mutex<Option<Channel<DropEvent>>>);

impl DropSink {
    /// Replaces the listener.
    pub fn set(&self, channel: Channel<DropEvent>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(channel);
        tracing::info!("a page listens for dropped files");
    }

    fn send(&self, event: DropEvent) {
        let mut slot = self.0.lock().unwrap_or_else(PoisonError::into_inner);

        if slot.is_none() {
            tracing::info!("dropped files arrived with no page listening");
        }

        // A closed channel means its page is gone.
        if slot.as_ref().is_some_and(|channel| channel.send(event).is_err()) {
            *slot = None;
        }
    }
}

/// Forwards the window's native drag and drop events to the registered listener.
pub fn forward<R: Runtime>(window: &WebviewWindow<R>) {
    let handle = window.app_handle().clone();

    // The main webview fills the window, so Tauri reports its drag and drop as window events.
    window.on_window_event(move |event| {
        let WindowEvent::DragDrop(drag) = event else {
            return;
        };
        let Some(state) = handle.try_state::<AppState>() else {
            return;
        };

        match drag {
            DragDropEvent::Enter { paths, .. } => {
                tracing::info!(files = paths.len(), "files dragged over the window");
                state.drops.send(DropEvent::Hover { active: true });
            }
            DragDropEvent::Leave => state.drops.send(DropEvent::Hover { active: false }),
            DragDropEvent::Drop { paths, .. } => {
                tracing::info!(files = paths.len(), "files dropped on the window");
                state.drops.send(DropEvent::Hover { active: false });
                read_in_background(&handle, paths.clone());
            }
            _ => {}
        }
    });
}

fn read_in_background<R: Runtime>(handle: &tauri::AppHandle<R>, paths: Vec<PathBuf>) {
    let handle = handle.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let (images, files, skipped) = read_dropped(&paths);

        if let Some(state) = handle.try_state::<AppState>() {
            state.drops.send(DropEvent::Dropped { images, files, skipped });
        }
    });
}
