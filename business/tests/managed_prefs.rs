//! Server-managed prefs: `save_ui` keeps project and binary state; binary overrides are validated.

use std::fs;
use std::path::{Path, PathBuf};

use harness_business::env::OsKind;
use harness_business::error::BusinessError;
use harness_business::model::{AppPrefs, BinaryOverride, HarnessId, MotionPref, ThemePref, UpdatePolicy};
use harness_business::prefs::PrefsStore;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn abs(name: &str) -> String {
    let root = if cfg!(windows) { "C:\\work" } else { "/work" };
    PathBuf::from(root).join(name).to_string_lossy().into_owned()
}

fn override_for(harness: HarnessId, path: &str) -> BinaryOverride {
    BinaryOverride {
        harness,
        path: path.to_owned(),
    }
}

/// A store holding server-managed state the webview must not be able to change.
fn store_with_server_state(dir: &TempDir) -> (PrefsStore, AppPrefs) {
    let store = PrefsStore::new(dir.path().to_path_buf());
    let stored = store
        .save(&AppPrefs {
            active_project: Some(abs("active")),
            recent_projects: vec![abs("active"), abs("older")],
            binary_overrides: vec![override_for(HarnessId::Omp, &abs("bin/omp"))],
            ..AppPrefs::default()
        })
        .unwrap();

    (store, stored)
}

#[test]
fn save_ui_stores_ui_fields_and_keeps_server_managed_fields() {
    let dir = TempDir::new().unwrap();
    let (store, stored) = store_with_server_state(&dir);

    let from_webview = AppPrefs {
        theme: ThemePref::Light,
        motion: MotionPref::Off,
        intro_enabled: false,
        sound_enabled: false,
        default_harness: Some(HarnessId::Pi),
        update_policy: UpdatePolicy::Off,
        onboarding_completed: true,
        approval_notice_seen: true,
        active_project: Some(abs("elsewhere")),
        recent_projects: vec![abs("elsewhere")],
        binary_overrides: vec![override_for(HarnessId::Omp, &abs("evil/omp"))],
        ..AppPrefs::default()
    };

    let saved = store.save_ui(&from_webview).unwrap();

    let expected = AppPrefs {
        theme: ThemePref::Light,
        motion: MotionPref::Off,
        intro_enabled: false,
        sound_enabled: false,
        default_harness: Some(HarnessId::Pi),
        update_policy: UpdatePolicy::Off,
        onboarding_completed: true,
        approval_notice_seen: true,
        ..stored
    };
    assert_eq!(saved, expected);
    assert_eq!(store.load(), expected);
}

#[test]
fn save_ui_cannot_clear_server_managed_fields() {
    let dir = TempDir::new().unwrap();
    let (store, stored) = store_with_server_state(&dir);

    let saved = store.save_ui(&AppPrefs::default()).unwrap();

    assert_eq!(saved.active_project, stored.active_project);
    assert_eq!(saved.recent_projects, stored.recent_projects);
    assert_eq!(saved.binary_overrides, stored.binary_overrides);
}

#[test]
fn turning_off_recent_projects_through_the_ui_clears_them() {
    let dir = TempDir::new().unwrap();
    let (store, stored) = store_with_server_state(&dir);

    let saved = store
        .save_ui(&AppPrefs {
            remember_recent_projects: false,
            ..stored.clone()
        })
        .unwrap();

    assert!(!saved.remember_recent_projects);
    assert!(saved.recent_projects.is_empty());
    assert_eq!(saved.active_project, stored.active_project);
    assert!(store.load().recent_projects.is_empty());
}

fn fake_binary(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, "#!/bin/sh\n").unwrap();
    path
}

#[test]
fn binary_override_replaces_the_previous_one_and_can_be_cleared() {
    let dir = TempDir::new().unwrap();
    let (store, _) = store_with_server_state(&dir);
    let bin = TempDir::new().unwrap();
    let omp = fake_binary(bin.path(), "omp");
    let pi = fake_binary(bin.path(), "pi");

    store
        .set_binary_override(OsKind::Linux, HarnessId::Pi, "pi", &pi)
        .unwrap();
    let saved = store
        .set_binary_override(OsKind::Linux, HarnessId::Omp, "omp", &omp)
        .unwrap();

    let path = |path: &Path| path.to_string_lossy().into_owned();
    assert_eq!(
        saved.binary_overrides,
        [
            override_for(HarnessId::Pi, &path(&pi)),
            override_for(HarnessId::Omp, &path(&omp))
        ]
    );

    let cleared = store.clear_binary_override(HarnessId::Omp).unwrap();

    assert_eq!(cleared.binary_overrides, [override_for(HarnessId::Pi, &path(&pi))]);
    assert_eq!(store.load(), cleared);
}

#[test]
fn binary_override_requires_the_harness_binary_name() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());
    let bin = TempDir::new().unwrap();
    let other = fake_binary(bin.path(), "omp-evil");

    let error = store
        .set_binary_override(OsKind::Linux, HarnessId::Omp, "omp", &other)
        .unwrap_err();

    assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
    assert!(store.load().binary_overrides.is_empty());
}

#[test]
fn binary_override_accepts_windows_launchers() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());
    let bin = TempDir::new().unwrap();
    let shim = fake_binary(bin.path(), "OMP.cmd");

    let saved = store
        .set_binary_override(OsKind::Windows, HarnessId::Omp, "omp", &shim)
        .unwrap();

    assert_eq!(saved.binary_overrides.len(), 1);
}

#[test]
fn binary_override_requires_an_existing_file() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());
    let bin = TempDir::new().unwrap();

    fs::create_dir(bin.path().join("pi")).unwrap();

    let missing = store.set_binary_override(OsKind::Linux, HarnessId::Omp, "omp", &bin.path().join("omp"));
    let directory = store.set_binary_override(OsKind::Linux, HarnessId::Pi, "pi", &bin.path().join("pi"));

    assert!(matches!(missing, Err(BusinessError::InvalidInput(_))), "{missing:?}");
    assert!(
        matches!(directory, Err(BusinessError::InvalidInput(_))),
        "{directory:?}"
    );
    assert!(store.load().binary_overrides.is_empty());
}
