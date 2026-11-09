//! The wire format of `AppPrefs`, the two memory copy switches ("Copy to remote", "Copy to this
//! computer"): both off by default, a file without them loads them off, and the webview's save
//! stores them.

use harness_business::model::AppPrefs;
use harness_business::prefs::PrefsStore;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

use super::prefs::load_file;

#[test]
fn default_prefs_serialize_with_camel_case_fields() {
    let json = serde_json::to_value(AppPrefs::default()).unwrap();

    assert_eq!(
        json,
        json!({
            "version": 1,
            "theme": "system",
            "motion": "full",
            "language": "en",
            "introEnabled": true,
            "soundEnabled": false,
            "showThinking": true,
            "showMessageUsage": true,
            "chatDefaults": [],
            "defaultHarness": null,
            "activeProject": null,
            "rememberRecentProjects": true,
            "recentProjects": [],
            "updatePolicy": "autoInstall",
            "checkCompanionUpdates": true,
            "checkHarnessUpdates": true,
            "autoUpdateExtensions": true,
            "autoRefreshCatalogs": true,
            "openAnyLink": false,
            "aiChatNames": false,
            "onboardingCompleted": false,
            "approvalNoticeSeen": false,
            "binaryOverrides": [],
            "memoryEnabled": true,
            "vectorSearch": true,
            "knowledgeEnabled": true,
            "knowledgeVectorSearch": true,
            "codeEnabled": true,
            "codeAutoInstallDone": false,
            "knowledgeRestartNoticeDone": false,
            "updatesTurnedOn": true,
            "memoryOptInNoticeDone": true,
            "keepRunningInTray": false,
            "chatAlerts": true,
            "alertSound": true,
            "memoryNotesAskDefault": false,
            "memoryScriptsAskDefault": false,
            "memoryCopyToRemote": false,
            "memoryCopyToLocal": false,
            "backgroundFetch": true,
            "limitBars": "used",
            "limitForecast": true,
            "limitGauge": "bars",
            "mainBranches": []
        })
    );
}

#[test]
fn the_copy_switches_are_off_in_a_file_that_never_had_them() {
    let loaded = load_file(r#"{ "version": 1, "theme": "dark" }"#);

    assert!(!loaded.memory_copy_to_remote);
    assert!(!loaded.memory_copy_to_local);
}

#[test]
fn the_copy_switches_are_stored_apart_and_the_webview_may_change_them() {
    let loaded = load_file(r#"{ "memoryCopyToRemote": true }"#);
    assert!(loaded.memory_copy_to_remote);
    assert!(!loaded.memory_copy_to_local);

    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());

    let saved = store
        .save_ui(&AppPrefs {
            memory_copy_to_local: true,
            ..AppPrefs::default()
        })
        .unwrap();

    assert!(!saved.memory_copy_to_remote);
    assert!(saved.memory_copy_to_local);
    assert!(store.load().memory_copy_to_local);
}
