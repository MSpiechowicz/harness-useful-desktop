//! App state with the update checks switched on or off.

use harness_business::model::AppPrefs;

use super::{TempDir, home_state};
use crate::state::AppState;

/// App state over `<dir>/home` whose saved preferences have the family update check
/// (`checkCompanionUpdates`) on or off.
pub fn state_with_update_checks(dir: &TempDir, check_companion_updates: bool) -> AppState {
    state_with_prefs(
        dir,
        AppPrefs {
            check_companion_updates,
            ..AppPrefs::default()
        },
    )
}

/// App state over `<dir>/home` whose saved preferences have the harness CLI update check
/// (`checkHarnessUpdates`) on or off.
pub fn state_with_harness_checks(dir: &TempDir, check_harness_updates: bool) -> AppState {
    state_with_prefs(
        dir,
        AppPrefs {
            check_harness_updates,
            ..AppPrefs::default()
        },
    )
}

fn state_with_prefs(dir: &TempDir, prefs: AppPrefs) -> AppState {
    let state = home_state(dir);
    state.prefs.with(|store| store.save(&prefs).expect("save prefs"));

    state
}
