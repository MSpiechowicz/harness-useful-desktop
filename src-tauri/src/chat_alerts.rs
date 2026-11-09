//! Where a chat's alerts end up. The hook engine decides when a chat raises one
//! (`harness_business::hooks`, see [`ChatAlerts::raise`]), this module delivers it:
//!
//! - The `chat_alerts` pref off: nothing at all.
//! - The main window is not in front: it asks for attention and an OS notification shows the chat's
//!   name and what happened. A click on it brings the window to the front and tells the page to open
//!   the chat ([`ChatAlertEvent::Open`]).
//! - The window is in front and shows the chat: nothing, the user sees it.
//! - The window is in front and shows another chat or another view: the page gets a toast event
//!   ([`ChatAlertEvent::Toast`]).
//!
//! Every alert that is shown or toasted also plays the chime while the `alert_sound` pref is on, at
//! most one per [`CHIME_EVERY`] however many chats raise one.
//!
//! Only chats the user started raise alerts: never a routine run's chat or a utility agent (see
//! `ChatSink::raises_alerts`). A notification carries the chat's name and its state, never a tool's
//! arguments or a command. The page reports only names a chat was given, never the opening words of
//! its first message, and the name is redacted before it shows (see `view::notification_body`). Only the chat's
//! key and the alert's kind are logged.
//!
//! The page tells the app which chat is on screen and what the open chats are called
//! ([`ChatAlerts::set_view`], the `chat_alerts_view` command).
//!
//! Alerts are delivered one after another by a single worker thread, started when the app attaches
//! ([`ChatAlerts::attach`]): two alerts of a chat arrive in the order they were raised, a delivery
//! that panics does not end the worker, and the queue is bounded ([`QUEUE_LEN`]), so raising an alert
//! never blocks the task that raises it.

mod host;
mod view;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_worker;

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use harness_business::model::{ChatAlertEvent, ChatAlertKind, ChatAlertsView};
use serde_json::Value;
use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime};

use self::host::{AppHost, Host};
use self::view::is_chat_key;
use crate::native_text::{NativeText, native_text};

/// The least time between two chimes.
pub const CHIME_EVERY: Duration = Duration::from_millis(1500);

/// The most alerts waiting for the delivery worker. Past it an alert is dropped: the worker is stuck
/// on a window or a notification service that does not answer.
pub const QUEUE_LEN: usize = 64;

/// The extra data key of an alert notification: the key of its chat.
pub const KEY_EXTRA: &str = "chatKey";

/// What an alert does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Nothing: alerts are off, or the user already looks at the chat.
    Silent,
    /// Ask for the window's attention and show an OS notification.
    Notify,
    /// Tell the page to show a toast.
    Toast,
}

/// What an alert of chat `key` does: `enabled` is the pref, `in_front` whether the main window is
/// shown, focused and not minimized, `shown` the chat the page shows (`None` on any other view).
pub fn route(enabled: bool, in_front: bool, shown: Option<&str>, key: &str) -> Route {
    if !enabled {
        return Route::Silent;
    }

    if !in_front {
        return Route::Notify;
    }

    if shown == Some(key) {
        return Route::Silent;
    }

    Route::Toast
}

/// Delivers the alerts of every chat.
#[derive(Default)]
pub struct ChatAlerts {
    shared: Arc<Shared>,
    /// Feeds the delivery worker; none until [`ChatAlerts::attach`] (at launch).
    queue: Mutex<Option<SyncSender<(String, ChatAlertKind)>>>,
    /// The alerts raised so far, for the tests of the code that raises them.
    #[cfg(test)]
    raised: Mutex<Vec<(String, ChatAlertKind)>>,
}

/// What a delivery needs. Shared with the delivery worker.
#[derive(Default)]
struct Shared {
    /// The page told about toasts and clicks.
    listener: Mutex<Option<Channel<ChatAlertEvent>>>,
    /// What the page last reported.
    view: Mutex<ChatAlertsView>,
    /// When the chime last played.
    chimed: Mutex<Option<Instant>>,
}

impl ChatAlerts {
    /// Delivers alerts through `app`'s window, prefs and notifications from now on.
    pub fn attach<R: Runtime>(&self, app: &AppHandle<R>) {
        self.attach_host(Arc::new(AppHost::new(app.clone())));
    }

    /// Delivers alerts through `host` from now on, with a new worker; the one a previous call
    /// started ends once its queue is empty.
    fn attach_host(&self, host: Arc<dyn Host>) {
        let (queue, alerts) = sync_channel(QUEUE_LEN);
        let shared = self.shared.clone();

        let worker = std::thread::Builder::new()
            .name("chat-alerts".to_owned())
            .spawn(move || shared.deliver_all(host.as_ref(), &alerts));

        match worker {
            Ok(worker) => drop(worker),
            Err(error) => {
                tracing::warn!(%error, "the chat alert worker did not start");
                return;
            }
        }

        *self.queue.lock().unwrap_or_else(PoisonError::into_inner) = Some(queue);
    }

    /// Sends the toasts and the notification clicks to `channel`, replacing the previous listener.
    pub fn listen(&self, channel: Channel<ChatAlertEvent>) {
        *self.shared.listener.lock().unwrap_or_else(PoisonError::into_inner) = Some(channel);
    }

    /// Keeps what the page reports about the chats, within bounds (see [`view::bounded`]).
    pub fn set_view(&self, view: ChatAlertsView) {
        *self.shared.view.lock().unwrap_or_else(PoisonError::into_inner) = view::bounded(view);
    }

    /// Chat `key` raises an alert of `kind`. Called once per alert, after the engine decided it is
    /// due, from a task that must not block: the alert is queued for the delivery worker, which
    /// delivers in the order raised. Before the app is attached it is only recorded.
    pub fn raise(&self, key: &str, kind: ChatAlertKind) {
        #[cfg(test)]
        self.raised
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((key.to_owned(), kind));

        tracing::debug!(session = %key, kind = ?kind, "chat alert raised");

        let queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner).clone();

        let Some(queue) = queue else {
            return;
        };

        match queue.try_send((key.to_owned(), kind)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => tracing::warn!(session = %key, "chat alert dropped: the queue is full"),
            Err(TrySendError::Disconnected(_)) => tracing::warn!(session = %key, "chat alert dropped: no worker"),
        }
    }

    /// The user clicked a notification that carried `extra` data: opens the chat it names, if any.
    /// A key that names no open chat is harmless, the page ignores it.
    pub fn on_notification_click(&self, extra: &HashMap<String, Value>) {
        if let Some(key) = clicked_chat(extra) {
            self.shared.send(ChatAlertEvent::Open { key });
        }
    }

    /// The alerts raised so far, oldest first.
    #[cfg(test)]
    pub fn raised(&self) -> Vec<(String, ChatAlertKind)> {
        self.raised
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// The chat a clicked notification with `extra` data names: only one of this module has it
/// ([`KEY_EXTRA`]), and only a key of the form chats get (see [`is_chat_key`]) counts.
pub fn clicked_chat(extra: &HashMap<String, Value>) -> Option<String> {
    extra
        .get(KEY_EXTRA)
        .and_then(Value::as_str)
        .filter(|key| is_chat_key(key))
        .map(str::to_owned)
}

impl Shared {
    /// Delivers the alerts of `alerts` in order until every sender is gone. A delivery that panics
    /// is logged and the next one still runs.
    fn deliver_all(&self, host: &dyn Host, alerts: &Receiver<(String, ChatAlertKind)>) {
        for (key, kind) in alerts {
            let delivered = catch_unwind(AssertUnwindSafe(|| self.deliver(host, &key, kind, Instant::now())));

            if delivered.is_err() {
                tracing::error!(session = %key, kind = ?kind, "a chat alert delivery panicked");
            }
        }
    }

    /// Does what the alert of chat `key` calls for at `now`.
    fn deliver(&self, host: &dyn Host, key: &str, kind: ChatAlertKind, now: Instant) {
        let prefs = host.prefs();
        let (shown, title) = self.view_of(key);

        match route(prefs.chat_alerts, host.in_front(), shown.as_deref(), key) {
            Route::Silent => return,
            Route::Notify => {
                host.attention();

                let text = native_text(&prefs.language);
                let body = title.as_deref().and_then(view::notification_body);

                host.notify(kind_title(text, kind), body.as_deref().unwrap_or(text.chat_open), key);
            }
            Route::Toast => self.send(ChatAlertEvent::Toast {
                key: key.to_owned(),
                kind,
            }),
        }

        if prefs.alert_sound && self.chime_due(now) {
            host.chime();
        }
    }

    /// The chat the page shows, and the title of chat `key`.
    fn view_of(&self, key: &str) -> (Option<String>, Option<String>) {
        let view = self.view.lock().unwrap_or_else(PoisonError::into_inner);

        (view.shown.clone(), view.titles.get(key).cloned())
    }

    /// Whether a chime may play at `now`, and counts it as played when it may.
    fn chime_due(&self, now: Instant) -> bool {
        let mut chimed = self.chimed.lock().unwrap_or_else(PoisonError::into_inner);

        if chimed.is_some_and(|last| now.saturating_duration_since(last) < CHIME_EVERY) {
            return false;
        }

        *chimed = Some(now);

        true
    }

    /// Sends `event` to the listener; a listener whose page is gone is dropped.
    fn send(&self, event: ChatAlertEvent) {
        let mut listener = self.listener.lock().unwrap_or_else(PoisonError::into_inner);

        if let Some(channel) = listener.as_ref()
            && channel.send(event).is_err()
        {
            *listener = None;
        }
    }
}

/// The notification title for an alert of `kind`.
fn kind_title(text: &NativeText, kind: ChatAlertKind) -> &'static str {
    match kind {
        ChatAlertKind::Finished => text.chat_finished,
        ChatAlertKind::NeedsYou => text.chat_needs_you,
        ChatAlertKind::Failed => text.chat_failed,
    }
}
