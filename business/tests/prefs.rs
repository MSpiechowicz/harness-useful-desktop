//! `PrefsStore`: defaults, corruption recovery, normalization and atomic writes.

use std::fs;
use std::path::PathBuf;

use harness_business::model::{AppPrefs, BinaryOverride, ChatDefaults, HarnessId, ThemePref};
use harness_business::prefs::{MAX_RECENT_PROJECTS, PREFS_FILE, PrefsOrigin, PrefsStore};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

/// An absolute path that is valid on the current OS.
fn abs(name: &str) -> String {
    let root = if cfg!(windows) { "C:\\work" } else { "/work" };
    PathBuf::from(root).join(name).to_string_lossy().into_owned()
}

fn file_names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn missing_file_loads_defaults() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().join("never-created"));

    assert_eq!(store.load(), AppPrefs::default());
}

#[test]
fn corrupt_file_is_moved_aside_and_defaults_are_returned() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(PREFS_FILE), "{ not json").unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());

    assert_eq!(store.load(), AppPrefs::default());

    let names = file_names(dir.path());
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(names[0].starts_with("prefs.json.corrupt-"), "{names:?}");
    assert_eq!(fs::read_to_string(dir.path().join(&names[0])).unwrap(), "{ not json");
}

#[test]
fn saved_prefs_round_trip_and_leave_no_temp_files() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().join("cfg"));
    let prefs = AppPrefs {
        theme: ThemePref::Dark,
        default_harness: Some(HarnessId::Omp),
        active_project: Some(abs("a")),
        recent_projects: vec![abs("a")],
        onboarding_completed: true,
        ..AppPrefs::default()
    };

    let stored = store.save(&prefs).unwrap();
    store.save(&stored).unwrap();

    assert_eq!(stored, prefs);
    assert_eq!(store.load(), prefs);
    assert_eq!(file_names(&dir.path().join("cfg")), [PREFS_FILE]);
}

#[cfg(unix)]
#[test]
fn prefs_file_and_directory_are_private() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().join("cfg"));
    store.save(&AppPrefs::default()).unwrap();

    let mode = |path: PathBuf| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(dir.path().join("cfg")), 0o700);
    assert_eq!(mode(store.path()), 0o600);
}

#[test]
fn unknown_fields_are_ignored_and_missing_fields_defaulted() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join(PREFS_FILE),
        r#"{ "version": 1, "theme": "light", "someFutureFlag": true }"#,
    )
    .unwrap();

    let loaded = PrefsStore::new(dir.path().to_path_buf()).load();

    assert_eq!(
        loaded,
        AppPrefs {
            theme: ThemePref::Light,
            ..AppPrefs::default()
        }
    );
}

#[test]
fn save_normalizes_recent_projects() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());

    let mut recent = vec![abs("b"), "relative/x".to_owned(), String::new(), abs("a"), abs("b")];
    recent.extend((0..20).map(|index| abs(&format!("p{index}"))));

    let stored = store
        .save(&AppPrefs {
            recent_projects: recent,
            ..AppPrefs::default()
        })
        .unwrap();

    assert_eq!(stored.recent_projects.len(), MAX_RECENT_PROJECTS);
    assert_eq!(stored.recent_projects[..3], [abs("b"), abs("a"), abs("p0")]);
}

#[test]
fn turning_off_recent_projects_clears_them() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());

    let stored = store
        .save(&AppPrefs {
            remember_recent_projects: false,
            recent_projects: vec![abs("a")],
            ..AppPrefs::default()
        })
        .unwrap();

    assert!(stored.recent_projects.is_empty());
    assert!(store.load().recent_projects.is_empty());
}

#[test]
fn save_drops_relative_active_project_and_normalizes_overrides() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());
    let entry = |harness, path: String| BinaryOverride { harness, path };

    let stored = store
        .save(&AppPrefs {
            version: 7,
            active_project: Some("relative".to_owned()),
            binary_overrides: vec![
                entry(HarnessId::Omp, abs("old-omp")),
                entry(HarnessId::Pi, "pi".to_owned()),
                entry(HarnessId::Omp, abs("new-omp")),
            ],
            ..AppPrefs::default()
        })
        .unwrap();

    assert_eq!(stored.version, 1);
    assert_eq!(stored.active_project, None);
    assert_eq!(stored.binary_overrides, [entry(HarnessId::Omp, abs("new-omp"))]);
}

#[test]
fn save_keeps_one_clean_chat_default_per_harness() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());
    let some = |value: &str| Some(value.to_owned());
    let entry = |harness, provider, model_id, thinking_level| ChatDefaults {
        harness,
        provider,
        model_id,
        thinking_level,
    };

    let stored = store
        .save(&AppPrefs {
            chat_defaults: vec![
                entry(HarnessId::Omp, some("anthropic"), some("old"), some("low")),
                // A model without its provider is dropped as a pair; a control character drops the level.
                entry(HarnessId::Pi, None, some("orphan"), some("hi\u{7}gh")),
                entry(HarnessId::Omp, some("minimax"), some("MiniMax-M3"), some("high")),
            ],
            ..AppPrefs::default()
        })
        .unwrap();

    assert_eq!(
        stored.chat_defaults,
        [
            entry(HarnessId::Pi, None, None, None),
            entry(HarnessId::Omp, some("minimax"), some("MiniMax-M3"), some("high")),
        ]
    );

    let long = "x".repeat(harness_business::prefs::MAX_CHAT_DEFAULT_LEN + 1);
    let stored = store
        .save(&AppPrefs {
            chat_defaults: vec![entry(HarnessId::Omp, some("p"), Some(long), some("high"))],
            ..AppPrefs::default()
        })
        .unwrap();
    assert_eq!(stored.chat_defaults, [entry(HarnessId::Omp, None, None, some("high"))]);
}

#[test]
fn load_checked_tells_a_missing_file_from_an_unusable_one() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());

    assert_eq!(store.load_checked(), (AppPrefs::default(), PrefsOrigin::Missing));

    let saved = store.save(&AppPrefs::default()).unwrap();
    assert_eq!(store.load_checked(), (saved, PrefsOrigin::Stored));

    fs::write(dir.path().join(PREFS_FILE), r#"{"version":1,"theme":"bogus"}"#).unwrap();
    assert_eq!(store.load_checked(), (AppPrefs::default(), PrefsOrigin::Unusable));

    // The corrupt file was moved aside; later loads still do not take it for "never stored".
    assert_eq!(store.load_checked().1, PrefsOrigin::Unusable);

    store.save(&AppPrefs::default()).unwrap();
    assert_eq!(store.load_checked().1, PrefsOrigin::Stored);
}
