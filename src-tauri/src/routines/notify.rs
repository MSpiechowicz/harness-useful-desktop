//! Tells the user about routine runs (see "Notifications" in `docs/routines-format.md`): a run
//! that waits for an answer, failed or needs attention always, one that finished at most once per
//! routine and [`FINISHED_EVERY`] (an hour counts from a notification that was shown). While the
//! main window is in front the OS notification is left out: the Routines view hears the same run
//! through `routines_listen` (`runFinished`, `needsYou`) and shows a toast instead.
//!
//! A "Routine needs you" notification carries its run id ([`NEEDS_YOU_KEY`], [`RUN_KEY`]), so a
//! click opens that run's chat ([`needs_you_run`]). The other notifications carry nothing and a
//! click only brings the window to the front. A platform that reports a click without the
//! notification's data opens nothing either.
//!
//! A notification holds the routine's name (valid by construction: no control or invisible
//! characters), never its prompt, command or output.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use harness_business::model::{RoutineRun, RunStatus};
use harness_business::signed_docs::is_doc_id;
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

use crate::native_text::{NativeText, native_text};
use crate::notifications::{main_window_in_front, request_attention, show_with_extra};
use crate::state::AppState;

/// How often a routine's "Routine finished" may show.
pub const FINISHED_EVERY: Duration = Duration::from_secs(60 * 60);

/// The extra data key of a "Routine needs you" notification (value [`NEEDS_YOU`]).
pub const NEEDS_YOU_KEY: &str = "harnessRoutineAlert";

/// The value of [`NEEDS_YOU_KEY`].
pub const NEEDS_YOU: &str = "needsYou";

/// The extra data key of the run a "Routine needs you" notification is about.
pub const RUN_KEY: &str = "harnessRoutineRun";

/// What happened to a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    /// It waits for an answer.
    NeedsYou,
    /// It failed.
    Failed,
    /// It cannot go on without the user.
    NeedsAttention,
    /// It went well.
    Finished,
}

/// One notification about a run of a routine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    /// What happened.
    pub kind: AlertKind,
    /// The routine.
    pub routine_id: String,
    /// The run.
    pub run_id: String,
    /// Its name when the run started.
    pub name: String,
}

impl Alert {
    /// Run `run_id` of routine `routine_id` (`name`) waits for an answer.
    pub fn needs_you(routine_id: &str, run_id: &str, name: &str) -> Self {
        Self {
            kind: AlertKind::NeedsYou,
            routine_id: routine_id.to_owned(),
            run_id: run_id.to_owned(),
            name: name.to_owned(),
        }
    }

    /// The notification for the end of `run`; `None` for a stopped one.
    pub fn of_run(run: &RoutineRun) -> Option<Self> {
        let kind = match run.record.status {
            RunStatus::Ok => AlertKind::Finished,
            RunStatus::Failed => AlertKind::Failed,
            RunStatus::NeedsAttention => AlertKind::NeedsAttention,
            RunStatus::Running | RunStatus::Stopped => return None,
        };

        Some(Self {
            kind,
            routine_id: run.routine_id.clone(),
            run_id: run.record.id.clone(),
            name: run.record.routine_name.clone(),
        })
    }

    /// The extra data the notification carries: the run of a "needs you" one, none otherwise.
    pub fn extra(&self) -> Vec<(&'static str, String)> {
        match self.kind {
            AlertKind::NeedsYou => vec![(NEEDS_YOU_KEY, NEEDS_YOU.to_owned()), (RUN_KEY, self.run_id.clone())],
            AlertKind::Failed | AlertKind::NeedsAttention | AlertKind::Finished => Vec::new(),
        }
    }

    /// The title and body in `text`'s language.
    pub fn texts(&self, text: &NativeText) -> (&'static str, String) {
        let name = &self.name;

        match self.kind {
            AlertKind::NeedsYou => (text.routine_needs_you, format!("{name} · {}", text.routine_answer)),
            AlertKind::Failed => (text.routine_failed, format!("{name} · {}", text.routine_see_run)),
            AlertKind::NeedsAttention => (text.routine_attention, format!("{name} · {}", text.routine_see_run)),
            AlertKind::Finished => (text.routine_finished, name.clone()),
        }
    }
}

/// Where alerts go.
pub type AlertSink = Arc<dyn Fn(&Alert) + Send + Sync>;

/// The run a clicked notification with `extra` data asks to open: only a "Routine needs you" one
/// names it ([`Alert::extra`]).
pub fn needs_you_run(extra: &HashMap<String, Value>) -> Option<String> {
    if extra.get(NEEDS_YOU_KEY).and_then(Value::as_str) != Some(NEEDS_YOU) {
        return None;
    }

    extra
        .get(RUN_KEY)
        .and_then(Value::as_str)
        .filter(|run_id| is_doc_id(run_id))
        .map(str::to_owned)
}

/// The alerts of the runtime: none until a sink is attached (at launch), and the last "Routine
/// finished" of each routine that was shown.
#[derive(Default)]
pub struct Alerts {
    sink: Mutex<Option<AlertSink>>,
    finished: Mutex<HashMap<String, Instant>>,
}

impl Alerts {
    /// Sends the alerts to `sink` from now on.
    pub fn attach(&self, sink: AlertSink) {
        *self.sink.lock().unwrap_or_else(PoisonError::into_inner) = Some(sink);
    }

    /// Sends `alert`, unless it is a "Routine finished" of a routine that had one shown less than
    /// [`FINISHED_EVERY`] ago.
    pub fn send(&self, alert: &Alert) {
        if alert.kind == AlertKind::Finished && !self.finished_due(&alert.routine_id, Instant::now()) {
            return;
        }

        let sink = self.sink.lock().unwrap_or_else(PoisonError::into_inner).clone();

        if let Some(sink) = sink {
            sink(alert);
        }
    }

    /// The sink showed `alert` as an OS notification: a "Routine finished" counts against its
    /// routine's hour from now. One left out (the window was in front) does not.
    pub fn shown(&self, alert: &Alert) {
        if alert.kind == AlertKind::Finished {
            let mut finished = self.finished.lock().unwrap_or_else(PoisonError::into_inner);
            finished.insert(alert.routine_id.clone(), Instant::now());
        }
    }

    /// Whether routine `id` may have a "Routine finished" at `now`.
    fn finished_due(&self, id: &str, now: Instant) -> bool {
        let finished = self.finished.lock().unwrap_or_else(PoisonError::into_inner);

        finished
            .get(id)
            .is_none_or(|last| now.saturating_duration_since(*last) >= FINISHED_EVERY)
    }
}

/// Shows `alert` as an OS notification unless the main window is in front (the view toasts
/// then), and tells the runtime's alerts it was shown. A run that needs the user also asks for the
/// window's attention. Reads the saved language on a blocking thread.
pub fn native<R: Runtime>(app: &AppHandle<R>, alert: &Alert) {
    let app = app.clone();
    let alert = alert.clone();

    drop(tauri::async_runtime::spawn_blocking(move || {
        if main_window_in_front(&app) {
            return;
        }

        if matches!(alert.kind, AlertKind::NeedsYou | AlertKind::NeedsAttention) {
            request_attention(&app);
        }

        let text = native_text(&crate::tray::saved_language(&app));
        let (title, body) = alert.texts(text);

        show_with_extra(&app, title, &body, &alert.extra());

        if let Some(state) = app.try_state::<AppState>() {
            state.routines.alerts.shown(&alert);
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alert(kind: AlertKind, id: &str) -> Alert {
        Alert {
            kind,
            routine_id: id.to_owned(),
            run_id: "0123456789abcdef0123456789abcdef".to_owned(),
            name: "Morning review".to_owned(),
        }
    }

    /// Alerts whose sink records what it got, and shows (`shows`) or leaves out each one.
    fn recording(shows: bool) -> (Arc<Alerts>, Arc<Mutex<Vec<Alert>>>) {
        let alerts = Arc::new(Alerts::default());
        let sent = Arc::new(Mutex::new(Vec::new()));
        let (seen, shown_to) = (sent.clone(), Arc::downgrade(&alerts));

        alerts.attach(Arc::new(move |alert: &Alert| {
            seen.lock().unwrap().push(alert.clone());

            if let (true, Some(alerts)) = (shows, shown_to.upgrade()) {
                alerts.shown(alert);
            }
        }));

        (alerts, sent)
    }

    #[test]
    fn titles_and_bodies_name_the_routine() {
        let text = native_text("en");

        assert_eq!(
            alert(AlertKind::NeedsYou, "a").texts(text),
            (
                "Routine needs you",
                "Morning review · Answer it in Harness Desktop".to_owned()
            )
        );
        assert_eq!(
            alert(AlertKind::Failed, "a").texts(text),
            (
                "Routine failed",
                "Morning review · See the run in Harness Desktop".to_owned()
            )
        );
        assert_eq!(
            alert(AlertKind::NeedsAttention, "a").texts(text),
            (
                "Routine needs attention",
                "Morning review · See the run in Harness Desktop".to_owned()
            )
        );
        assert_eq!(
            alert(AlertKind::Finished, "a").texts(text),
            ("Routine finished", "Morning review".to_owned())
        );
    }

    #[test]
    fn a_finished_routine_is_notified_once_an_hour_and_failures_always() {
        let (alerts, sent) = recording(true);

        for _ in 0..3 {
            alerts.send(&alert(AlertKind::Finished, "a"));
            alerts.send(&alert(AlertKind::Failed, "a"));
        }
        alerts.send(&alert(AlertKind::Finished, "b"));

        let kinds: Vec<(AlertKind, String)> = sent
            .lock()
            .unwrap()
            .iter()
            .map(|alert| (alert.kind, alert.routine_id.clone()))
            .collect();

        assert_eq!(
            kinds,
            [
                (AlertKind::Finished, "a".to_owned()),
                (AlertKind::Failed, "a".to_owned()),
                (AlertKind::Failed, "a".to_owned()),
                (AlertKind::Failed, "a".to_owned()),
                (AlertKind::Finished, "b".to_owned()),
            ]
        );

        let later = Instant::now() + FINISHED_EVERY;
        assert!(alerts.finished_due("a", later), "an hour later it shows again");
    }

    #[test]
    fn a_finished_notification_left_out_for_the_window_keeps_the_hour() {
        let (alerts, sent) = recording(false);

        alerts.send(&alert(AlertKind::Finished, "a"));
        alerts.send(&alert(AlertKind::Finished, "a"));

        assert_eq!(
            sent.lock().unwrap().len(),
            2,
            "nothing was shown, so the next one may show"
        );

        alerts.shown(&alert(AlertKind::Finished, "a"));
        alerts.send(&alert(AlertKind::Finished, "a"));
        assert_eq!(sent.lock().unwrap().len(), 2, "one was shown this hour");
    }

    #[test]
    fn only_a_needs_you_notification_names_a_run_to_open() {
        let extra = |alert: &Alert| -> HashMap<String, Value> {
            alert
                .extra()
                .into_iter()
                .map(|(key, value)| (key.to_owned(), Value::String(value)))
                .collect()
        };

        let needs_you = alert(AlertKind::NeedsYou, "a");
        assert_eq!(needs_you_run(&extra(&needs_you)), Some(needs_you.run_id.clone()));

        for kind in [AlertKind::Failed, AlertKind::NeedsAttention, AlertKind::Finished] {
            assert_eq!(needs_you_run(&extra(&alert(kind, "a"))), None, "{kind:?}");
        }

        assert_eq!(needs_you_run(&HashMap::new()), None, "a script request notification");

        let mut forged = extra(&needs_you);
        forged.insert(RUN_KEY.to_owned(), Value::String("../x".to_owned()));
        assert_eq!(needs_you_run(&forged), None);
    }
}
