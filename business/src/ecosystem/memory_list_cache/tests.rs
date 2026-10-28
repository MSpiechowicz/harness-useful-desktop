use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::tempdir;

use super::*;

fn page(total: u32, offline: bool) -> MemoryNotesPage {
    serde_json::from_value(json!({
        "availability": { "kind": "available" },
        "total": total,
        "kinds": [],
        "scopes": null,
        "places": null,
        "combinedFilters": true,
        "notes": [],
        "hiddenCount": null,
        "localCounts": null,
        "offline": offline,
        "cachedAt": null,
    }))
    .unwrap()
}

#[test]
fn an_offline_page_is_the_last_live_page_of_the_same_request() {
    let dir = tempdir().unwrap();

    settle(dir.path(), "first", page(22, false));
    settle(dir.path(), "first", page(23, false));

    let shown = settle(dir.path(), "first", page(0, true));

    assert_eq!(shown.total, 23);
    assert!(shown.offline);
    assert!(shown.cached_at.is_some());
}

#[test]
fn an_offline_page_of_a_request_never_read_stays_empty() {
    let dir = tempdir().unwrap();

    settle(dir.path(), "first", page(22, false));

    let shown = settle(dir.path(), "second", page(0, true));

    assert_eq!(shown.total, 0);
    assert!(shown.offline);
    assert_eq!(shown.cached_at, None);
}

#[test]
fn an_unreadable_page_is_not_kept_over_a_good_one() {
    let dir = tempdir().unwrap();

    settle(dir.path(), "first", page(22, false));

    let failed = MemoryNotesPage {
        availability: MemoryScriptsAvailability::Failed {
            message: "x".to_owned(),
        },
        ..page(0, false)
    };
    settle(dir.path(), "first", failed);

    assert_eq!(settle(dir.path(), "first", page(0, true)).total, 22);
}

#[test]
fn a_file_of_another_version_is_no_copy() {
    let dir = tempdir().unwrap();

    settle(dir.path(), "first", page(22, false));

    let path = file_of(dir.path(), "notes", "first");
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("\"version\":1", "\"version\":2");
    fs::write(&path, text).unwrap();

    assert_eq!(settle(dir.path(), "first", page(0, true)).total, 0);
}

#[test]
fn only_the_newest_files_stay() {
    let dir = tempdir().unwrap();

    for index in 0..=MAX_FILES {
        settle(dir.path(), &index.to_string(), page(1, false));
    }

    let kept = fs::read_dir(dir.path().join(CACHE_DIR).join("notes")).unwrap().count();

    assert_eq!(kept, MAX_FILES);
}
