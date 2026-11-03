//! The check of the plan limits when an agent starts a subagent during a run.
//!
//! A subagent starting is the moment a run gets expensive, so the app looks at the limits again
//! ([`harness_business::ecosystem::read_limits`]) and, when a window or budget newly crossed a
//! level ([`harness_business::ecosystem::LimitLevels`]: 80, then 95 percent) since the chat was
//! last told, steers the running turn once:
//!
//! ```text
//! <harness-hook-followup name="Harness Dashboard">Limit close: Claude 5-hour window 91% used,
//! resets 14:00. Spawn fewer agents, or tell the user before you continue.</harness-hook-followup>
//! ```
//!
//! - Only while the chat's Harness Dashboard hook is active (enabled, approved, its member there).
//! - At most one check per [`MIN_GAP`] per chat, and a level is never told twice in a chat. What
//!   the prompt's own Harness Dashboard step showed counts as told.
//! - The steer is sent as the hook's own message ([`SendOrigin::Hook`]): it is no user prompt, and
//!   it is dropped when the run settled meanwhile. A check that fails sends nothing.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use harness_business::ecosystem::{Dashboard, Gauge, LimitLevels, close_gauges, limit_steer};
use harness_business::env::Env;
use harness_business::hooks::{HookSet, ResolvedPart, SendOrigin, compose_follow_up, has_seed_hook};
use harness_business::model::{AgentAction, HookSeed};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use tokio::sync::mpsc::UnboundedReceiver;

use super::ChatHooks;
use crate::agents::Agents;
use crate::hooks::HookRuntime;

/// The least time between two checks of one chat.
pub const MIN_GAP: Duration = Duration::from_secs(60);

/// The hook id and name of the steer.
const STEER_ID: &str = "harness-desktop:limits";
const STEER_NAME: &str = "Harness Dashboard";

/// What a chat was told about its limits, and when the limits were last read for it.
#[derive(Default)]
pub struct LimitsWatch {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    levels: LimitLevels,
    checked: Option<Instant>,
}

impl LimitsWatch {
    /// The chat was shown `gauges` (the prompt's step).
    pub fn told(&self, gauges: &[Gauge]) {
        self.lock().levels.told(gauges);
    }

    /// True when a check may run at `now`: none ran within [`MIN_GAP`]. The check counts as made.
    pub fn due(&self, now: Instant) -> bool {
        let mut inner = self.lock();

        if inner.checked.is_some_and(|last| now.duration_since(last) < MIN_GAP) {
            return false;
        }

        inner.checked = Some(now);
        true
    }

    /// The gauges of `gauges` that crossed a level the chat was not told yet; they count as told.
    pub fn crossed(&self, gauges: &[Gauge]) -> Vec<Gauge> {
        self.lock().levels.crossed(gauges)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // Plain values: a panic cannot leave them half-written.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Everything the check of one chat needs.
pub(super) struct Watch {
    pub runtime: Arc<HookRuntime>,
    pub agents: Arc<Agents>,
    pub env: Arc<Env>,
    pub dashboard: Arc<Dashboard>,
    pub chat: Arc<ChatHooks>,
}

impl Watch {
    /// Checks the limits for each subagent the chat's sink reports, until the sink is gone.
    pub(super) async fn run(self, mut started: UnboundedReceiver<()>) {
        while started.recv().await.is_some() {
            self.check().await;
        }
    }

    async fn check(&self) {
        if !self.chat.limits.due(Instant::now()) {
            return;
        }

        let hooks = self.runtime.hook_set(self.chat.project().as_deref()).await;

        if !has_seed_hook(&hooks, HookSeed::DashboardLimits) {
            return;
        }

        let settles = self.chat.settles_seen();

        let Some(reading) = super::limits_cache::limits_reading(&self.dashboard, &self.env).await else {
            return;
        };

        // The run ended while the dashboard answered: a steer would start another.
        if self.chat.settles_seen() != settles {
            return;
        }

        let gauges = close_gauges(&reading, &self.chat.limit_providers(), self.chat.project().as_deref());
        let crossed = self.chat.limits.crossed(&gauges);

        let Some(text) = limit_steer(&crossed, Timestamp::now(), &TimeZone::system()) else {
            return;
        };

        self.steer(&text, settles).await;
    }

    /// Steers the running turn with `text`, unless the run settled meanwhile.
    async fn steer(&self, text: &str, settles: u64) {
        let part = ResolvedPart::new(STEER_ID, STEER_NAME, text);

        let Some(message) = compose_follow_up(&[part]) else {
            return;
        };

        let mut session = self.chat.session.lock().await;

        if self.chat.settles_seen() != settles {
            return;
        }

        let action = AgentAction::Steer {
            message,
            images: Vec::new(),
        };
        session.on_send(&HookSet::empty(), &action, SendOrigin::Hook);

        match self.agents.manager().send(self.chat.key(), action).await {
            Ok(reply) => {
                session.follow_up_replied(&reply);
                tracing::info!(session = %self.chat.key(), "limit steer sent");
            }
            Err(error) => tracing::warn!(session = %self.chat.key(), code = ?error.code(), "limit steer not sent"),
        }
    }
}
