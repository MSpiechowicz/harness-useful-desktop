//! The system tray (menu bar on macOS) icon: the Harness Desktop mark in its own family gradient, so
//! it stays distinguishable next to the other Harness products. Its menu shows or hides the main
//! window and quits; quitting goes through the normal exit path, so agent sessions are stopped first
//! (see `lifecycle`). A left click on the icon brings the window back where the platform reports
//! clicks (Linux tray indicators only open the menu). The menu is in the saved UI language and is
//! rebuilt when the language changes (see [`relabel`]).

use std::sync::atomic::{AtomicBool, Ordering, fence};
use std::sync::{Mutex, TryLockError};

use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Runtime};

use crate::native_text::native_text;
use crate::state::AppState;
use crate::window::{MAIN_WINDOW, focus_main_window};

/// 64 px render of `icons/source/tray.svg` (the family mark from ../harness-useful-logo, desktop gradient).
const ICON: &[u8] = include_bytes!("../icons/tray.png");

const SHOW: &str = "show";
const HIDE: &str = "hide";
const QUIT: &str = "quit";

const TRAY_ID: &str = "main";

/// Held by the [`relabel`] that is rebuilding the menu, so relabels never overlap (see [`coalesce`]).
/// Only worker threads take it, and only with `try_lock`: nothing ever waits on it.
static RELABEL: Mutex<()> = Mutex::new(());

/// Set by every [`relabel`] request; cleared by the one holding [`RELABEL`] before each rebuild.
static RELABEL_PENDING: AtomicBool = AtomicBool::new(false);

/// Adds the tray icon. Failing to create it is not fatal: the app works without a tray.
/// Call after [`AppState`] is managed: the menu reads the saved language from its prefs.
pub fn create_tray<R: Runtime>(app: &App<R>) {
    // On Linux the indicator library is loaded on first use, and `libappindicator-sys` panics when no
    // usable copy loads (missing, or built against a newer GLib than the AppImage bundles).
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build(app))) {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => tracing::warn!(%error, "cannot create the tray icon"),
        Err(_) => tracing::warn!("cannot create the tray icon: the system tray library failed to load"),
    }
}

fn build<R: Runtime>(app: &App<R>) -> tauri::Result<TrayIcon<R>> {
    let menu = menu(app.handle(), &saved_language(app.handle()))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON)?)
        .tooltip("Harness Desktop")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                focus_main_window(tray.app_handle());
            }
        })
        .build(app)
}

/// True when the tray icon was created, so a hidden window can be shown again from it.
pub fn has_tray<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.tray_by_id(TRAY_ID).is_some()
}

/// Rebuilds the tray menu in the saved UI language. Failing is not fatal: the old labels stay.
///
/// Blocking (reads the prefs file and waits for the main thread): call it off the async runtime and
/// never on the main thread. Returns at once when another call is already rebuilding the menu: that
/// call rebuilds it once more for this request, reading the language then rather than taking it from
/// its caller, so after quick successive changes the menu ends in the last saved language.
pub fn relabel<R: Runtime>(app: &AppHandle<R>) {
    // No tray when it could not be created at startup.
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };

    coalesce(
        &RELABEL,
        &RELABEL_PENDING,
        || saved_language(app),
        |language| {
            if let Err(error) = menu(app, &language).and_then(|menu| tray.set_menu(Some(menu))) {
                tracing::warn!(%error, "cannot relabel the tray menu");
            }
        },
    );
}

/// Requests `apply(read())` and runs it unless another call holding `turn` is already applying, in
/// which case that call applies once more for this request. Calls never overlap, a burst of requests
/// during one apply costs one more apply, and when every change to what `read` returns is followed
/// by a call, the last value applied is the latest one: `read` runs after `pending` is cleared, and
/// the holder checks `pending` again after giving up `turn`.
fn coalesce<T>(turn: &Mutex<()>, pending: &AtomicBool, read: impl Fn() -> T, mut apply: impl FnMut(T)) {
    pending.store(true, Ordering::SeqCst);

    loop {
        // Pairs with the fence after the unlock below: either this `try_lock` finds `turn` free, or
        // the holder that has it sees `pending` once it lets go.
        fence(Ordering::SeqCst);

        let held = match turn.try_lock() {
            Ok(held) => held,
            // A panic in an earlier `apply` leaves nothing to repair: the guarded value is `()`.
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return,
        };

        while pending.swap(false, Ordering::SeqCst) {
            apply(read());
        }

        drop(held);
        fence(Ordering::SeqCst);

        // A request made between the last check and the unlock found `turn` taken and left it to us.
        if !pending.load(Ordering::SeqCst) {
            return;
        }
    }
}

/// The saved UI language. Locks the prefs only while reading them.
pub(crate) fn saved_language<R: Runtime>(app: &AppHandle<R>) -> String {
    app.state::<AppState>().prefs.with(|store| store.load()).language
}

/// The tray menu in the UI language `lang`. The item ids stay the same in every language.
fn menu<R: Runtime>(app: &AppHandle<R>, lang: &str) -> tauri::Result<Menu<R>> {
    let text = native_text(lang);

    let show = MenuItem::with_id(app, SHOW, text.show_window, true, None::<&str>)?;
    let hide = MenuItem::with_id(app, HIDE, text.hide_window, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, QUIT, text.quit, true, None::<&str>)?;

    Menu::with_items(app, &[&show, &hide, &separator, &quit])
}

fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        SHOW => focus_main_window(app),
        HIDE => {
            if let Some(window) = app.get_webview_window(MAIN_WINDOW)
                && let Err(error) = window.hide()
            {
                tracing::warn!(%error, "cannot hide the main window");
            }
        }
        QUIT => app.exit(0),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Mutex, mpsc};
    use std::thread;
    use std::time::Duration;

    use super::coalesce;

    /// Each writer stores its value, then requests an apply of what is stored with a delay that makes
    /// earlier writers finish last; the applied value still ends equal to the stored one.
    #[test]
    fn overlapping_calls_end_with_the_latest_value_applied() {
        let turn = Mutex::new(());
        let pending = AtomicBool::new(false);
        let stored = Mutex::new(0_u32);
        let applied = Mutex::new(0_u32);

        thread::scope(|scope| {
            for value in 1..=8_u32 {
                let (turn, pending, stored, applied) = (&turn, &pending, &stored, &applied);

                scope.spawn(move || {
                    *stored.lock().unwrap() = value;

                    coalesce(
                        turn,
                        pending,
                        || *stored.lock().unwrap(),
                        |latest| {
                            thread::sleep(Duration::from_millis(u64::from(9 - value)));
                            *applied.lock().unwrap() = latest;
                        },
                    );
                });
            }
        });

        assert_eq!(*applied.lock().unwrap(), *stored.lock().unwrap());
    }

    /// While one apply is in progress, many more requests return at once and together cost a
    /// single further apply, which picks up the latest stored value.
    #[test]
    fn requests_during_an_apply_are_coalesced_into_one_more() {
        let turn = Mutex::new(());
        let pending = AtomicBool::new(false);
        let stored = Mutex::new(0_u32);
        let applied = Mutex::new(0_u32);
        let applies = AtomicUsize::new(0);
        let read = || *stored.lock().unwrap();
        let record = |latest| {
            applies.fetch_add(1, Ordering::SeqCst);
            *applied.lock().unwrap() = latest;
        };

        let (started, wait_started) = mpsc::channel();
        let (release, wait_release) = mpsc::channel::<()>();

        thread::scope(|scope| {
            let (turn, pending, stored) = (&turn, &pending, &stored);

            scope.spawn(move || {
                *stored.lock().unwrap() = 1;
                let mut first = true;

                coalesce(turn, pending, read, |latest| {
                    if first {
                        first = false;
                        started.send(()).unwrap();
                        wait_release.recv().unwrap();
                    }

                    record(latest);
                });
            });

            wait_started.recv().unwrap();

            let requests: Vec<_> = (2..=33_u32)
                .map(|value| {
                    scope.spawn(move || {
                        *stored.lock().unwrap() = value;
                        coalesce(turn, pending, read, record);
                    })
                })
                .collect();

            for request in requests {
                request.join().unwrap();
            }

            assert_eq!(applies.load(Ordering::SeqCst), 0, "applied during the first apply");
            release.send(()).unwrap();
        });

        assert_eq!(applies.load(Ordering::SeqCst), 2);
        assert_eq!(*applied.lock().unwrap(), *stored.lock().unwrap());
    }
}
