//! Server-managed prefs: `save_ui` keeps project and binary state; binary overrides are validated.

use std::fs;
use std::path::{Path, PathBuf};

use harness_business::env::OsKind;
use harness_business::error::BusinessError;
use harness_business::model::{
    AppPrefs, BinaryOverride, HarnessId, IpcErrorCode, MainBranchChoice, MotionPref, ThemePref, UpdatePolicy,
};
use harness_business::prefs::{MAX_MAIN_BRANCHES, PREFS_FILE, PrefsStore};
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

/// `base` with every UI-managed field changed from its default, as the webview would send it.
fn with_ui_changes(base: AppPrefs) -> AppPrefs {
    AppPrefs {
        theme: ThemePref::Light,
        motion: MotionPref::Off,
        language: "pl".to_owned(),
        intro_enabled: false,
        sound_enabled: false,
        default_harness: Some(HarnessId::Pi),
        update_policy: UpdatePolicy::Off,
        check_companion_updates: false,
        auto_update_extensions: false,
        auto_refresh_catalogs: false,
        open_any_link: true,
        ai_chat_names: true,
        onboarding_completed: true,
        approval_notice_seen: true,
        keep_running_in_tray: true,
        memory_notes_ask_default: true,
        memory_scripts_ask_default: true,
        background_fetch: false,
        ..base
    }
}

#[test]
fn save_ui_stores_ui_fields_and_keeps_server_managed_fields() {
    let dir = TempDir::new().unwrap();
    let (store, stored) = store_with_server_state(&dir);

    let from_webview = with_ui_changes(AppPrefs {
        active_project: Some(abs("elsewhere")),
        recent_projects: vec![abs("elsewhere")],
        binary_overrides: vec![override_for(HarnessId::Omp, &abs("evil/omp"))],
        ..AppPrefs::default()
    });

    let saved = store.save_ui(&from_webview).unwrap();

    let expected = with_ui_changes(stored);
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
fn save_ui_cannot_switch_project_memory() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());

    let saved = store
        .save_ui(&AppPrefs {
            memory_enabled: false,
            ..AppPrefs::default()
        })
        .unwrap();
    assert!(saved.memory_enabled);

    store.update(|prefs| prefs.memory_enabled = false).unwrap();
    assert!(!store.save_ui(&AppPrefs::default()).unwrap().memory_enabled);
}

#[test]
fn save_ui_cannot_switch_vector_search() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());

    let saved = store
        .save_ui(&AppPrefs {
            vector_search: false,
            ..AppPrefs::default()
        })
        .unwrap();
    assert!(saved.vector_search);

    store.update(|prefs| prefs.vector_search = false).unwrap();
    assert!(!store.save_ui(&AppPrefs::default()).unwrap().vector_search);
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
    fs::write(&path, "#!/bin/sh\n").unwrap(); // guard-allow: installer-shell
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

fn pick(project: &str, branch: &str) -> MainBranchChoice {
    MainBranchChoice {
        project: abs(project),
        branch: branch.to_owned(),
    }
}

#[test]
fn save_ui_cannot_change_the_picked_main_branches() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());
    store
        .set_main_branch(Path::new(&abs("project")), Some("develop"))
        .unwrap();

    let saved = store
        .save_ui(&AppPrefs {
            main_branches: vec![pick("project", "evil"), pick("other", "main")],
            ..AppPrefs::default()
        })
        .unwrap();
    assert_eq!(saved.main_branches, [pick("project", "develop")]);

    let cleared = store.save_ui(&AppPrefs::default()).unwrap();
    assert_eq!(cleared.main_branches, [pick("project", "develop")]);
}

#[test]
fn save_ui_stores_the_background_fetch_switch() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());

    let off = store
        .save_ui(&AppPrefs {
            background_fetch: false,
            ..AppPrefs::default()
        })
        .unwrap();
    assert!(!off.background_fetch);
    assert!(!store.load().background_fetch);

    assert!(store.save_ui(&AppPrefs::default()).unwrap().background_fetch);
}

#[test]
fn set_main_branch_replaces_a_pick_clears_it_and_validates_its_input() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());
    let project = PathBuf::from(abs("project"));

    store.set_main_branch(&project, Some("develop")).unwrap();
    store.set_main_branch(Path::new(&abs("other")), Some("main")).unwrap();
    let replaced = store.set_main_branch(&project, Some("trunk")).unwrap();
    assert_eq!(
        replaced.main_branches,
        [pick("other", "main"), pick("project", "trunk")]
    );

    let cleared = store.set_main_branch(&project, None).unwrap();
    assert_eq!(cleared.main_branches, [pick("other", "main")]);
    assert_eq!(store.load().main_branches, cleared.main_branches);

    for branch in ["-x", "", "a b", "HEAD"] {
        let error = store.set_main_branch(&project, Some(branch)).unwrap_err();
        assert_eq!(error.code(), IpcErrorCode::InvalidInput, "{branch:?}");
    }

    let relative = store.set_main_branch(Path::new("relative"), Some("main")).unwrap_err();
    assert_eq!(relative.code(), IpcErrorCode::InvalidInput);
    assert_eq!(store.load().main_branches, cleared.main_branches);
}

#[test]
fn save_keeps_one_valid_main_branch_per_project() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());

    let saved = store
        .save(&AppPrefs {
            main_branches: vec![
                pick("one", "develop"),
                pick("two", "main"),
                pick("one", "trunk"),
                pick("three", "-evil"),
                pick("four", "a..b"),
                MainBranchChoice {
                    project: "relative/path".to_owned(),
                    branch: "main".to_owned(),
                },
            ],
            ..AppPrefs::default()
        })
        .unwrap();

    // The last entry per project wins and keeps its position among the survivors.
    assert_eq!(saved.main_branches, [pick("two", "main"), pick("one", "trunk")]);
    assert_eq!(saved.picked_main_branch(&abs("one")), Some("trunk"));
    assert_eq!(saved.picked_main_branch(&abs("nowhere")), None);
    assert_eq!(store.load().main_branches, saved.main_branches);
}

#[test]
fn main_branch_picks_are_capped_keeping_the_newest() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());
    let picks: Vec<MainBranchChoice> = (0..MAX_MAIN_BRANCHES + 5)
        .map(|number| pick(&format!("project-{number}"), "main"))
        .collect();

    let saved = store
        .save(&AppPrefs {
            main_branches: picks.clone(),
            ..AppPrefs::default()
        })
        .unwrap();

    assert_eq!(saved.main_branches.len(), MAX_MAIN_BRANCHES);
    assert_eq!(saved.main_branches.first(), Some(&picks[5]));
    assert_eq!(saved.main_branches.last(), picks.last());
}

/// The prefs loaded from a `prefs.json` holding `json`.
fn load_file(json: &str) -> AppPrefs {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(PREFS_FILE), json).unwrap();

    PrefsStore::new(dir.path().to_path_buf()).load()
}

#[test]
fn background_fetch_is_on_for_a_fresh_install_and_for_a_file_without_the_key() {
    let dir = TempDir::new().unwrap();
    assert!(
        PrefsStore::new(dir.path().join("never-created"))
            .load()
            .background_fetch
    );

    assert!(load_file(r#"{"version":1,"theme":"dark"}"#).background_fetch);
    assert!(!load_file(r#"{"version":1,"backgroundFetch":false}"#).background_fetch);
}
