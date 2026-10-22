//! M5 fixtures for the update banner and Settings → Updates (`ui/src/lib/ipc/mock/fixtures/updates/`).
//! `update_available.json` and `no_update.json` are `update_check` results; the release is
//! synthetic (a real one needs the published manifest). `progress.json` is the sequence of
//! `update_install` progress reports [`ProgressTracker`] produces for a 2 MiB download arriving in
//! 64 KiB chunks.

use harness_business::model::{UpdateInfo, UpdateProgress};
use harness_business::update::ProgressTracker;
use serde::Serialize;
use serde_json::Value;

fn to_json(value: impl Serialize) -> Value {
    serde_json::to_value(value).expect("DTOs always serialize")
}

const NOTES: &str = "## 0.2.0 (2026-10-08)\n\n### Features\n\n* **chat:** pick the thinking level per session\n* **updates:** check for updates at startup and install them in the app\n\n### Bug Fixes\n\n* **changes:** keep the selected file when the diff refreshes\n";

fn update_available() -> Option<UpdateInfo> {
    Some(UpdateInfo::announced(
        "0.2.0".to_owned(),
        "0.1.0".to_owned(),
        Some("2026-10-08T18:42:11Z".to_owned()),
        Some(NOTES.to_owned()),
    ))
}

fn progress() -> Vec<UpdateProgress> {
    const CHUNK: usize = 64 * 1024;
    const TOTAL: u64 = 2 * 1024 * 1024;

    let mut tracker = ProgressTracker::default();
    let chunks = TOTAL / CHUNK as u64;

    let mut reports: Vec<UpdateProgress> = (0..chunks).filter_map(|_| tracker.chunk(CHUNK, Some(TOTAL))).collect();
    reports.push(tracker.installing());
    reports.push(tracker.done());
    reports
}

/// Every M5 fixture path (relative to the fixtures directory) with its content.
pub fn m5_fixtures() -> Vec<(&'static str, Value)> {
    vec![
        ("updates/update_available.json", to_json(update_available())),
        ("updates/no_update.json", to_json(None::<UpdateInfo>)),
        ("updates/progress.json", to_json(progress())),
    ]
}
