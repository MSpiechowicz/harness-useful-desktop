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
        && prefs.check_harness_updates
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

#[test]
fn the_opt_in_notice_is_done_for_a_fresh_install_and_due_for_an_existing_one() {
    let fresh = TempDir::new().unwrap();
    let (loaded, origin) = PrefsStore::new(fresh.path().to_path_buf()).load_checked();
    assert_eq!(origin, PrefsOrigin::Missing);
    assert!(
        loaded.memory_opt_in_notice_done,
        "a fresh install begins with saving off"
    );

    let (_dir, store) = store_with(r#"{ "version": 1, "theme": "light" }"#);
    let (existing, origin) = store.load_checked();
    assert_eq!(origin, PrefsOrigin::Stored);
    assert!(!existing.memory_opt_in_notice_done, "an existing install sees it once");
}

#[test]
fn marking_the_opt_in_notice_done_persists_and_the_ui_cannot_undo_it() {
    let (_dir, store) = store_with(r#"{ "version": 1 }"#);

    store.update(|prefs| prefs.memory_opt_in_notice_done = true).unwrap();
    assert!(store.load().memory_opt_in_notice_done);

    let mut from_ui = store.load();
    from_ui.memory_opt_in_notice_done = false;
    store.save_ui(&from_ui).unwrap();
    assert!(store.load().memory_opt_in_notice_done, "only the server marks it");
}

#[test]
fn an_unreadable_prefs_file_keeps_the_opt_in_notice_due() {
    let (_dir, store) = store_with("{ not json");

    let (loaded, origin) = store.load_checked();
    assert_eq!(origin, PrefsOrigin::Unusable);
    assert!(!loaded.memory_opt_in_notice_done);

    // The next save writes a readable file that still owes the notice.
    store.update(|_| {}).unwrap();
    assert!(!store.load().memory_opt_in_notice_done);
}

/// A store with no prefs file, next to a Harness Memory data folder that holds a database when
/// `database` is set.
fn fresh_store(database: bool) -> (TempDir, TempDir, PrefsStore) {
    let prefs_dir = TempDir::new().unwrap();
    let memory = TempDir::new().unwrap();

    if database {
        fs::write(memory.path().join("memory.sqlite3"), b"SQLite format 3\0").unwrap();
    }

    let store = PrefsStore::new(prefs_dir.path().to_path_buf()).with_memory_data_dir(Some(memory.path().to_path_buf()));

    (prefs_dir, memory, store)
}

#[test]
fn a_new_prefs_file_owes_the_opt_in_notice_when_harness_memory_has_a_database() {
    let (_prefs, _memory, store) = fresh_store(true);

    let (loaded, origin) = store.load_checked();
    assert_eq!(origin, PrefsOrigin::Missing);
    assert!(
        !loaded.memory_opt_in_notice_done,
        "saving was on before: tell the person once"
    );

    // The first save writes the decision, which later loads keep.
    store.update(|_| {}).unwrap();
    assert!(!store.load().memory_opt_in_notice_done);
}

#[test]
fn a_new_prefs_file_owes_no_notice_without_a_database() {
    let (_prefs, memory, store) = fresh_store(false);
    let (loaded, origin) = store.load_checked();
    assert_eq!(origin, PrefsOrigin::Missing);
    assert!(loaded.memory_opt_in_notice_done);

    // A folder named like the database is no database.
    fs::create_dir(memory.path().join("memory.sqlite3")).unwrap();
    assert!(store.load().memory_opt_in_notice_done);

    // Neither does an unknown data folder.
    let unknown = PrefsStore::new(TempDir::new().unwrap().path().to_path_buf());
    assert!(unknown.load().memory_opt_in_notice_done);
    assert!(
        PrefsStore::new(TempDir::new().unwrap().path().to_path_buf())
            .with_memory_data_dir(None)
            .load()
            .memory_opt_in_notice_done
    );
}

#[test]
fn the_database_decides_only_when_the_prefs_file_is_first_created() {
    // Created without a database: a database that shows up later changes nothing.
    let (_prefs, memory, store) = fresh_store(false);
    store.update(|_| {}).unwrap();
    fs::write(memory.path().join("memory.sqlite3"), b"SQLite format 3\0").unwrap();
    assert!(store.load().memory_opt_in_notice_done);

    // An existing file keeps its own value, a database or not.
    let (prefs_dir, memory, _store) = fresh_store(true);
    fs::write(
        prefs_dir.path().join(PREFS_FILE),
        r#"{ "version": 1, "memoryOptInNoticeDone": true }"#,
    )
    .unwrap();
    let store = PrefsStore::new(prefs_dir.path().to_path_buf()).with_memory_data_dir(Some(memory.path().to_path_buf()));
    let (stored, origin) = store.load_checked();
    assert_eq!(origin, PrefsOrigin::Stored);
    assert!(stored.memory_opt_in_notice_done);

    fs::write(prefs_dir.path().join(PREFS_FILE), r#"{ "version": 1 }"#).unwrap();
    assert!(
        !store.load().memory_opt_in_notice_done,
        "a file without the key owes it, as before"
    );
}

#[test]
fn the_ui_cannot_clear_a_notice_that_a_database_made_due() {
    let (_prefs, _memory, store) = fresh_store(true);
    store.update(|_| {}).unwrap();

    let mut from_ui = store.load();
    from_ui.memory_opt_in_notice_done = true;
    store.save_ui(&from_ui).unwrap();

    assert!(!store.load().memory_opt_in_notice_done, "only the server marks it");
}

#[test]
fn a_file_from_before_the_harness_check_takes_it_from_the_family_check() {
    let (_on_dir, on) = store_with(r#"{ "version": 1, "checkCompanionUpdates": true }"#);
    let (_off_dir, off) = store_with(r#"{ "version": 1, "checkCompanionUpdates": false }"#);

    assert!(on.load().check_harness_updates);
    assert!(!off.load().check_harness_updates);
}

#[test]
fn a_stored_harness_check_choice_is_kept_whatever_the_family_check_says() {
    let (_on_dir, on) = store_with(r#"{ "version": 1, "checkCompanionUpdates": false, "checkHarnessUpdates": true }"#);
    let (_off_dir, off) =
        store_with(r#"{ "version": 1, "checkCompanionUpdates": true, "checkHarnessUpdates": false }"#);

    assert!(on.load().check_harness_updates);
    assert!(!off.load().check_harness_updates);
}
