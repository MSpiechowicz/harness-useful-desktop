//! The system tray (menu bar on macOS) icon: the Harness Desktop mark in its own family gradient, so
//! it stays distinguishable next to the other Harness products. Its menu shows or hides the main
//! window and quits; quitting goes through the normal exit path, so agent sessions are stopped first
//! (see `lifecycle`). A left click on the icon brings the window back where the platform reports
//! clicks (Linux tray indicators only open the menu).

use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Runtime};

use crate::window::{MAIN_WINDOW, focus_main_window};

/// 64 px render of `icons/source/tray.svg` (the family mark from ../harness-useful-logo, desktop gradient).
const ICON: &[u8] = include_bytes!("../icons/tray.png");

const SHOW: &str = "show";
const HIDE: &str = "hide";
const QUIT: &str = "quit";

/// Adds the tray icon. Failing to create it is not fatal: the app works without a tray.
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
    let show = MenuItem::with_id(app, SHOW, "Show Harness Desktop", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, HIDE, "Hide window", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, QUIT, "Quit Harness Desktop", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &separator, &quit])?;

    TrayIconBuilder::with_id("main")
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
