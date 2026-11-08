//! App state with the update checks switched on or off.

use harness_business::model::AppPrefs;

use super::{TempDir, home_state};
use crate::state::AppState;

/// App state over `<dir>/home` whose saved preferences have the update checks
/// (`checkCompanionUpdates`) on or off.
pub fn state_with_update_checks(dir: &TempDir, check_companion_updates: bool) -> AppState {
    let state = home_state(dir);
    state.prefs.with(|store| {
        store
            .save(&AppPrefs {
                check_companion_updates,
                ..AppPrefs::default()
            })
            .expect("save prefs")
    });

    state
}
