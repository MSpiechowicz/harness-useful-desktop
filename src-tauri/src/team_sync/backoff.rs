//! How long the team refresh loop waits after refreshes that failed.

use std::time::Duration;

use super::TEAM_REFRESH;

/// The longest wait between refreshes while they keep failing.
pub const TEAM_REFRESH_MAX: Duration = Duration::from_secs(10 * 60);

/// The refreshes that failed in a row.
#[derive(Debug, Default)]
pub struct Backoff {
    failures: u32,
}

impl Backoff {
    /// The wait before the next refresh, given whether the one that just ran `worked`:
    /// [`TEAM_REFRESH`], doubled for each failure in a row after the first, up to [`TEAM_REFRESH_MAX`].
    pub fn after(&mut self, worked: bool) -> Duration {
        self.failures = if worked { 0 } else { self.failures.saturating_add(1) };

        wait(self.failures)
    }
}

fn wait(failures: u32) -> Duration {
    let doublings = failures.saturating_sub(1).min(8);

    (TEAM_REFRESH * 2_u32.pow(doublings)).min(TEAM_REFRESH_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wait_doubles_with_each_failed_refresh_up_to_the_longest_and_a_good_one_ends_it() {
        let mut backoff = Backoff::default();

        let minutes: Vec<u64> = [true, false, false, false, false, false, false, false]
            .into_iter()
            .map(|worked| backoff.after(worked).as_secs() / 60)
            .collect();

        assert_eq!(minutes, [1, 1, 2, 4, 8, 10, 10, 10]);
        assert_eq!(backoff.after(true), TEAM_REFRESH);
    }
}
