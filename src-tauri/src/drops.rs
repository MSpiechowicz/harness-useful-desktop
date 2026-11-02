//! Files dragged onto the window.
//!
//! The webview's own drag and drop cannot read a file the file manager drags in (it only sees a
//! `file://` text), so the window's native drag and drop is on (`dragDropEnabled`) and its paths
//! stay in Rust: the images among them are read here and sent to the webview over the channel
//! `drop_listen` registered. The webview never names a path. The paths of the last drop also wait
//! for the Knowledge view to take them as documents (`knowledge_drop_intake`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use harness_business::dropped::read_dropped;
use harness_business::model::{DropEvent, DropSpot};
use tauri::ipc::Channel;
use tauri::{DragDropEvent, Manager, PhysicalPosition, Runtime, WebviewWindow, WindowEvent};

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

/// Where a native drag is in CSS pixels. The platform reports physical pixels from the top left of the webview,
/// which fills the window, and the page measures in logical ones: the scale factor turns one into the other.
/// `None` for a scale the window cannot have.
fn spot(position: PhysicalPosition<f64>, scale: f64, items: usize, folder: bool) -> Option<DropSpot> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }

    let logical = position.to_logical::<f64>(scale);

    if !logical.x.is_finite() || !logical.y.is_finite() {
        return None;
    }

    // Saturating casts: a position past the range of a page is clamped, not wrapped.
    #[allow(clippy::cast_possible_truncation)]
    let (x, y) = (logical.x.round() as i32, logical.y.round() as i32);

    Some(DropSpot {
        x,
        y,
        items: u32::try_from(items).unwrap_or(u32::MAX),
        folder,
    })
}

/// Whether a drag is one folder, which an import takes and a single document replacement cannot.
fn is_one_folder(paths: &[PathBuf]) -> bool {
    matches!(paths, [only] if Path::new(only).is_dir())
}

/// Forwards the window's native drag and drop events to the registered listener.
pub fn forward<R: Runtime>(window: &WebviewWindow<R>) {
    let handle = window.app_handle().clone();
    let label = window.label().to_owned();
    // How many items the drag carries: only its first event names them.
    let dragged = Arc::new(AtomicUsize::new(0));
    let folder = Arc::new(AtomicBool::new(false));

    // The main webview fills the window, so Tauri reports its drag and drop as window events.
    window.on_window_event(move |event| {
        let WindowEvent::DragDrop(drag) = event else {
            return;
        };
        let Some(state) = handle.try_state::<AppState>() else {
            return;
        };

        let scale = handle
            .get_webview_window(&label)
            .and_then(|window| window.scale_factor().ok())
            .unwrap_or(1.0);

        match drag {
            DragDropEvent::Enter { paths, position } => {
                tracing::info!(files = paths.len(), "files dragged over the window");
                dragged.store(paths.len(), Ordering::Relaxed);
                folder.store(is_one_folder(paths), Ordering::Relaxed);
                let at = spot(*position, scale, paths.len(), folder.load(Ordering::Relaxed));
                state.drops.send(DropEvent::Hover { active: true, at });
            }
            DragDropEvent::Over { position } => {
                let at = spot(
                    *position,
                    scale,
                    dragged.load(Ordering::Relaxed),
                    folder.load(Ordering::Relaxed),
                );
                state.drops.send(DropEvent::Hover { active: true, at });
            }
            DragDropEvent::Leave => {
                dragged.store(0, Ordering::Relaxed);
                folder.store(false, Ordering::Relaxed);
                state.drops.send(DropEvent::Hover {
                    active: false,
                    at: None,
                });
            }
            DragDropEvent::Drop { paths, position } => {
                tracing::info!(files = paths.len(), "files dropped on the window");
                dragged.store(0, Ordering::Relaxed);
                folder.store(false, Ordering::Relaxed);
                state.drops.send(DropEvent::Hover {
                    active: false,
                    at: None,
                });
                // The Knowledge view takes the same drop as documents (`knowledge_drop_intake`).
                state.knowledge_intakes.stash_drop(paths);
                read_in_background(
                    &handle,
                    paths.clone(),
                    spot(*position, scale, paths.len(), is_one_folder(paths)),
                );
            }
            _ => {}
        }
    });
}

fn read_in_background<R: Runtime>(handle: &tauri::AppHandle<R>, paths: Vec<PathBuf>, at: Option<DropSpot>) {
    let handle = handle.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let (images, files, skipped) = read_dropped(&paths);

        if let Some(state) = handle.try_state::<AppState>() {
            state.drops.send(DropEvent::Dropped {
                images,
                files,
                skipped,
                at,
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_physical_position_becomes_css_pixels_by_the_scale() {
        let at = spot(PhysicalPosition::new(1500.0, 750.4), 1.5, 1, false).expect("a position");

        assert_eq!((at.x, at.y, at.items), (1000, 500, 1));
    }

    #[test]
    fn a_position_at_scale_one_stays() {
        let at = spot(PhysicalPosition::new(12.0, 7.0), 1.0, 3, false).expect("a position");

        assert_eq!((at.x, at.y, at.items), (12, 7, 3));
    }

    #[test]
    fn a_scale_no_window_has_gives_no_position() {
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                spot(PhysicalPosition::new(10.0, 10.0), scale, 1, false),
                None,
                "{scale}"
            );
        }
    }

    #[test]
    fn a_position_that_is_not_a_number_gives_none() {
        assert_eq!(spot(PhysicalPosition::new(f64::NAN, 1.0), 1.0, 1, false), None);
    }
}
