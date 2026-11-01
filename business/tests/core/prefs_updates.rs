//! The one-time switch of every update setting to automatic (`PrefsStore::turn_updates_on_once`).

use std::fs;

use harness_business::model::{AppPrefs, UpdatePolicy};
use harness_business::prefs::{PREFS_FILE, PrefsOrigin, PrefsStore};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn store_with(json: &str) -> (TempDir, PrefsStore) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(PREFS_FILE), json).unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());

    (dir, store)
}

fn all_automatic(prefs: &AppPrefs) -> bool {
    prefs.update_policy == UpdatePolicy::AutoInstall
        && prefs.check_companion_updates
        && prefs.auto_update_extensions
        && prefs.auto_refresh_catalogs
}

#[test]
fn an_install_with_updates_off_gets_them_all_on_once_and_keeps_its_other_choices() {
    let (_dir, store) = store_with(
        r#"{ "version": 1, "theme": "light", "updatePolicy": "off", "checkCompanionUpdates": false,
             "autoUpdateExtensions": false, "autoRefreshCatalogs": false, "backgroundFetch": false }"#,
    );

    assert!(store.turn_updates_on_once().unwrap());

    let after = store.load();
    assert!(all_automatic(&after));
    assert!(after.updates_turned_on);
    assert!(!after.background_fetch, "only the update settings change");
    assert_eq!(after.theme, harness_business::model::ThemePref::Light);
}

#[test]
fn a_choice_made_after_the_switch_is_kept_on_the_next_launch() {
    let (_dir, store) = store_with(r#"{ "version": 1, "autoUpdateExtensions": false }"#);
    store.turn_updates_on_once().unwrap();

    store
        .update(|prefs| {
            prefs.update_policy = UpdatePolicy::StartupAsk;
            prefs.auto_update_extensions = false;
        })
        .unwrap();

    assert!(!store.turn_updates_on_once().unwrap());

    let after = store.load();
    assert_eq!(after.update_policy, UpdatePolicy::StartupAsk);
    assert!(!after.auto_update_extensions);
}

#[test]
fn a_fresh_install_refreshes_catalogs_and_updates_extensions() {
    let dir = TempDir::new().unwrap();
    let (loaded, origin) = PrefsStore::new(dir.path().to_path_buf()).load_checked();

    assert_eq!(origin, PrefsOrigin::Missing);
    assert!(loaded.auto_update_extensions);
    assert!(loaded.auto_refresh_catalogs);
}

#[test]
fn a_fresh_install_starts_automatic_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());

    assert!(!store.turn_updates_on_once().unwrap());
    assert!(!dir.path().join(PREFS_FILE).exists());

    let (prefs, origin) = store.load_checked();
    assert_eq!(origin, PrefsOrigin::Missing);
    assert!(all_automatic(&prefs) && prefs.updates_turned_on);
}

#[test]
fn unreadable_prefs_stay_off_until_a_readable_file_exists() {
    let (dir, store) = store_with("{ not json");

    assert!(!store.turn_updates_on_once().unwrap());
    assert!(!store.load().auto_update_extensions, "fails closed");

    // The next save writes a readable file that has not had the switch yet.
    store.update(|_| {}).unwrap();
    assert!(store.turn_updates_on_once().unwrap());
    assert!(all_automatic(&store.load()));
    drop(dir);
}
