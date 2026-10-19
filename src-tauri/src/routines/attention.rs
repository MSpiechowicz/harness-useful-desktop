//! Questions of an agent run (UI requests: dialogs, tool approvals) that wait for the user.
//!
//! A run waits for an answer while one of its requests is open ("Needs you"). While no window
//! shows its chat the wait is limited: after [`UI_WAIT`] the run gives up (its requests are
//! cancelled, its agent stopped, the run needs attention). An attached window answers like any
//! chat and has no limit. The wait starts again when the last window leaves.
//!
//! Debug builds read the wait from [`UI_WAIT_VAR`] (seconds), to try it out quickly.

use std::collections::BTreeSet;
use std::time::Duration;

use harness_business::env::Env;
use tokio::time::Instant;

/// How long a question waits while no window shows the run.
pub const UI_WAIT: Duration = Duration::from_secs(10 * 60);

/// Debug builds only: the wait in seconds.
pub const UI_WAIT_VAR: &str = "HARNESS_ROUTINE_UI_WAIT_SECS";

/// The message of a run nobody answered.
pub const UNANSWERED_MESSAGE: &str = "Nobody answered the agent's question within 10 minutes";

/// The wait for an answer: [`UI_WAIT`], or [`UI_WAIT_VAR`] in a debug build.
pub fn ui_wait(env: &Env) -> Duration {
    if cfg!(debug_assertions)
        && let Some(secs) = env.var(UI_WAIT_VAR).and_then(|value| value.trim().parse::<u64>().ok())
    {
        return Duration::from_secs(secs);
    }

    UI_WAIT
}

/// The open requests of one run, whether a window shows it, and since when it waits unseen.
#[derive(Debug)]
pub struct Attention {
    open: BTreeSet<String>,
    attached: bool,
    wait: Duration,
    unseen_since: Option<Instant>,
}

impl Attention {
    /// No request open, no window, a wait of `wait`.
    pub fn new(wait: Duration) -> Self {
        Self {
            open: BTreeSet::new(),
            attached: false,
            wait,
            unseen_since: None,
        }
    }

    /// The agent asked request `id`.
    pub fn asked(&mut self, id: String) {
        self.open.insert(id);
        self.rearm();
    }

    /// Request `id` was answered or withdrawn.
    pub fn closed(&mut self, id: &str) {
        self.open.remove(id);
        self.rearm();
    }

    /// A window attached or left.
    pub fn attached(&mut self, attached: bool) {
        self.attached = attached;
        self.rearm();
    }

    /// True while a request is open.
    pub fn waiting(&self) -> bool {
        !self.open.is_empty()
    }

    /// The open requests.
    pub fn open(&self) -> impl Iterator<Item = &String> {
        self.open.iter()
    }

    /// When the run gives up, while it waits unseen.
    pub fn deadline(&self) -> Option<Instant> {
        self.unseen_since.map(|since| since + self.wait)
    }

    fn rearm(&mut self) {
        if !self.waiting() || self.attached {
            self.unseen_since = None;
        } else if self.unseen_since.is_none() {
            self.unseen_since = Some(Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAIT: Duration = Duration::from_secs(600);

    #[test]
    fn an_open_question_waits_only_while_no_window_shows_the_run() {
        let mut attention = Attention::new(WAIT);
        assert!(!attention.waiting());
        assert_eq!(attention.deadline(), None);

        attention.asked("a".to_owned());
        let first = attention.deadline().expect("waits unseen");
        assert!(attention.waiting());

        attention.asked("b".to_owned());
        assert_eq!(
            attention.deadline(),
            Some(first),
            "a second question keeps the first wait"
        );

        attention.attached(true);
        assert_eq!(attention.deadline(), None, "a window answers without a limit");
        assert!(attention.waiting());

        attention.attached(false);
        assert!(attention.deadline().is_some_and(|deadline| deadline >= first));

        attention.closed("a");
        attention.closed("b");
        assert!(!attention.waiting());
        assert_eq!(attention.deadline(), None);
    }

    #[test]
    fn the_wait_is_ten_minutes_unless_a_debug_build_is_told_otherwise() {
        let home = std::env::temp_dir();

        assert_eq!(ui_wait(&Env::for_tests(&home)), UI_WAIT);

        let told = Env::for_tests(&home).with_var(UI_WAIT_VAR, "2");
        let expected = if cfg!(debug_assertions) {
            Duration::from_secs(2)
        } else {
            UI_WAIT
        };
        assert_eq!(ui_wait(&told), expected);
    }
}
