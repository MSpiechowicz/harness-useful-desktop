//! The last Harness Dashboard limits reading, kept for the Harness Dashboard step and the mid-run
//! check.
//!
//! The pooled `harness-dashboard mcp` stops 30 s after its last query, and a cold one takes about
//! 4 s to answer. Asking it on every message would either miss the reading (a short time limit) or
//! hold the message back (a long one). So a message reads this cache at once, and a reading older
//! than [`REFRESH_AFTER`] is refreshed in the background for the next one. Only when there is no
//! usable reading (none yet, or older than [`USABLE_FOR`]) does a message wait, at most
//! [`FIRST_WAIT`]. A chat warms the cache when it starts ([`warm`]), so its first message finds a
//! reading. The dashboard itself refreshes its readings every few minutes, so a reading this old
//! says nothing different.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use harness_business::ecosystem::{Dashboard, DashboardReading, read_limits};
use harness_business::env::Env;
use tokio::sync::Mutex;

/// A reading older than this is refreshed in the background.
pub const REFRESH_AFTER: Duration = Duration::from_secs(60);

/// A reading older than this is not used: the message waits for a new one.
pub const USABLE_FOR: Duration = Duration::from_secs(5 * 60);

/// How long a message waits when there is no usable reading.
pub const FIRST_WAIT: Duration = Duration::from_secs(6);

/// The last reading and whether a refresh is running.
#[derive(Default)]
pub struct LimitsCache {
    last: Mutex<Option<(Instant, DashboardReading)>>,
    refreshing: AtomicBool,
}

static CACHE: std::sync::OnceLock<Arc<LimitsCache>> = std::sync::OnceLock::new();

/// The app's cache.
fn cache() -> Arc<LimitsCache> {
    CACHE.get_or_init(|| Arc::new(LimitsCache::default())).clone()
}

/// The limits reading for a message or a mid-run check (see the module docs); `None` when the
/// dashboard is missing or gives none in time.
pub async fn limits_reading(dashboard: &Arc<Dashboard>, env: &Arc<Env>) -> Option<DashboardReading> {
    let fetch = fetcher(dashboard.clone(), env.clone());

    cache().reading(Instant::now(), fetch).await
}

/// Starts a refresh in the background when the cache has no fresh reading, so the first message
/// of a chat finds one.
pub fn warm(dashboard: &Arc<Dashboard>, env: &Arc<Env>) {
    let cache = cache();
    let fetch = fetcher(dashboard.clone(), env.clone());

    tauri::async_runtime::spawn(async move { cache.refresh_if_due(Instant::now(), fetch).await });
}

fn fetcher(
    dashboard: Arc<Dashboard>,
    env: Arc<Env>,
) -> impl FnOnce() -> std::pin::Pin<Box<dyn Future<Output = Option<DashboardReading>> + Send>> + Send + 'static {
    move || Box::pin(async move { read_limits(&dashboard, &env).await })
}

impl LimitsCache {
    /// The cached reading when it is usable (refreshed in the background past [`REFRESH_AFTER`]),
    /// else a new one from `fetch`, waited for at most [`FIRST_WAIT`].
    pub async fn reading<F, Fut>(self: &Arc<Self>, now: Instant, fetch: F) -> Option<DashboardReading>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Option<DashboardReading>> + Send + 'static,
    {
        let cached = self.last.lock().await.clone();

        if let Some((at, reading)) = cached
            && now.saturating_duration_since(at) < USABLE_FOR
        {
            if now.saturating_duration_since(at) >= REFRESH_AFTER {
                let cache = self.clone();
                tauri::async_runtime::spawn(async move { cache.refresh(fetch).await });
            }

            return Some(reading);
        }

        tokio::time::timeout(FIRST_WAIT, self.fetch_now(fetch))
            .await
            .ok()
            .flatten()
    }

    /// Refreshes when there is no reading younger than [`REFRESH_AFTER`].
    pub async fn refresh_if_due<F, Fut>(&self, now: Instant, fetch: F)
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<DashboardReading>>,
    {
        let fresh = self
            .last
            .lock()
            .await
            .as_ref()
            .is_some_and(|(at, _)| now.saturating_duration_since(*at) < REFRESH_AFTER);

        if !fresh {
            self.refresh(fetch).await;
        }
    }

    /// One refresh at a time: a second one while another runs does nothing.
    async fn refresh<F, Fut>(&self, fetch: F)
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<DashboardReading>>,
    {
        if self.refreshing.swap(true, Ordering::AcqRel) {
            return;
        }

        self.fetch_now(fetch).await;
        self.refreshing.store(false, Ordering::Release);
    }

    /// Asks `fetch` and keeps what it answered.
    async fn fetch_now<F, Fut>(&self, fetch: F) -> Option<DashboardReading>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<DashboardReading>>,
    {
        let reading = fetch().await?;

        *self.last.lock().await = Some((Instant::now(), reading.clone()));
        Some(reading)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use serde_json::json;

    use super::*;

    fn reading(percent: u32) -> DashboardReading {
        DashboardReading {
            limits: json!({ "windows": [{ "provider": "claude", "window": "7d", "usedPercent": percent }] }),
            budgets: serde_json::Value::Null,
        }
    }

    fn counting(
        calls: &Arc<AtomicUsize>,
        percent: u32,
    ) -> impl FnOnce() -> std::future::Ready<Option<DashboardReading>> + Send + 'static {
        let calls = calls.clone();

        move || {
            calls.fetch_add(1, Ordering::SeqCst);
            std::future::ready(Some(reading(percent)))
        }
    }

    #[tokio::test]
    async fn the_first_message_waits_for_a_reading_and_later_ones_use_the_cache() {
        let cache = Arc::new(LimitsCache::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let now = Instant::now();

        assert_eq!(cache.reading(now, counting(&calls, 86)).await, Some(reading(86)));
        assert_eq!(cache.reading(now, counting(&calls, 10)).await, Some(reading(86)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_reading_past_its_use_is_fetched_again_before_the_message() {
        let cache = Arc::new(LimitsCache::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let start = Instant::now();

        cache.reading(start, counting(&calls, 86)).await;
        let later = start + USABLE_FOR + Duration::from_secs(1);

        assert_eq!(cache.reading(later, counting(&calls, 91)).await, Some(reading(91)));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn warming_fills_the_cache_once_and_skips_while_it_is_fresh() {
        let cache = Arc::new(LimitsCache::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let now = Instant::now();

        cache.refresh_if_due(now, counting(&calls, 86)).await;
        cache.refresh_if_due(now, counting(&calls, 86)).await;

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(cache.reading(now, counting(&calls, 10)).await, Some(reading(86)));
    }
}
