//! The updater rules: which configured public keys enable updates, what an announced release
//! shows, and how download progress is reported.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use harness_business::model::{MAX_UPDATE_NOTES_BYTES, UpdateInfo, UpdatePhase, UpdateProgress};
use harness_business::update::{PUBKEY_PLACEHOLDER, ProgressTracker, UNKNOWN_SIZE_REPORT_STEP, configured_pubkey};

/// A configured key as `tauri signer generate` writes it: the base64 of the whole `.pub` file.
/// The key is the public example from the minisign documentation.
fn pubkey_file(key_line: &str) -> String {
    STANDARD.encode(format!(
        "untrusted comment: minisign public key 37629A1A86B6E9F1\n{key_line}\n"
    ))
}

const EXAMPLE_KEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";

#[test]
fn a_minisign_public_key_enables_updates() {
    let configured = pubkey_file(EXAMPLE_KEY);

    assert_eq!(configured_pubkey(Some(&configured)), Some(configured.as_str()));
}

#[test]
fn builds_without_a_usable_key_have_no_updates() {
    let truncated_key = pubkey_file(&EXAMPLE_KEY[..40]);
    let raw_key_line = EXAMPLE_KEY.to_owned();
    let not_base64 = "-----BEGIN PUBLIC KEY-----".to_owned();

    for value in [
        None,
        Some(""),
        Some(PUBKEY_PLACEHOLDER),
        Some(truncated_key.as_str()),
        Some(raw_key_line.as_str()),
        Some(not_base64.as_str()),
    ] {
        assert_eq!(configured_pubkey(value), None, "{value:?}");
    }
}

#[test]
fn announced_releases_drop_empty_notes_and_cut_long_ones() {
    let announced =
        |notes: Option<String>| UpdateInfo::announced("0.2.0".to_owned(), "0.1.0".to_owned(), None, notes).notes;

    assert_eq!(announced(None), None);
    assert_eq!(announced(Some(" \n".to_owned())), None);
    assert_eq!(announced(Some("## Fixes".to_owned())).as_deref(), Some("## Fixes"));

    let long = "é".repeat(MAX_UPDATE_NOTES_BYTES);
    let cut = announced(Some(long)).expect("notes");
    assert!(cut.len() <= MAX_UPDATE_NOTES_BYTES);
    assert!(cut.len() > MAX_UPDATE_NOTES_BYTES - 2);
    assert!(cut.chars().all(|char| char == 'é'));
}

fn downloading(downloaded: u64, total: Option<u64>) -> UpdateProgress {
    UpdateProgress {
        phase: UpdatePhase::Downloading,
        downloaded,
        total,
    }
}

#[test]
fn a_known_size_is_reported_once_per_percent_and_at_the_end() {
    let mut tracker = ProgressTracker::default();
    let total = Some(10_000);

    let reports: Vec<UpdateProgress> = (0..100).filter_map(|_| tracker.chunk(100, total)).collect();

    assert_eq!(reports.len(), 100);
    assert_eq!(reports[0], downloading(100, total));
    assert_eq!(reports[99], downloading(10_000, total));

    let mut tracker = ProgressTracker::default();
    let reports: Vec<UpdateProgress> = (0..1000).filter_map(|_| tracker.chunk(10, total)).collect();

    assert_eq!(reports.len(), 101, "the first chunk and every 100 bytes after it");
    assert_eq!(reports.last(), Some(&downloading(10_000, total)));
}

#[test]
fn an_unknown_size_is_reported_every_step() {
    let mut tracker = ProgressTracker::default();
    let chunk = 64 * 1024;
    let chunks = 4 * UNKNOWN_SIZE_REPORT_STEP / chunk;

    let reports: Vec<UpdateProgress> = (0..chunks)
        .filter_map(|_| tracker.chunk(chunk as usize, None))
        .collect();

    let expected: Vec<UpdateProgress> = [
        chunk,
        chunk + UNKNOWN_SIZE_REPORT_STEP,
        chunk + 2 * UNKNOWN_SIZE_REPORT_STEP,
        chunk + 3 * UNKNOWN_SIZE_REPORT_STEP,
    ]
    .into_iter()
    .map(|downloaded| downloading(downloaded, None))
    .collect();
    assert_eq!(reports, expected);
}

#[test]
fn a_size_the_download_outgrows_becomes_unknown() {
    let mut tracker = ProgressTracker::default();

    assert_eq!(tracker.chunk(80, Some(100)), Some(downloading(80, Some(100))));
    assert_eq!(tracker.chunk(20, Some(100)), Some(downloading(100, Some(100))));
    assert_eq!(
        tracker.chunk(UNKNOWN_SIZE_REPORT_STEP as usize, Some(100)),
        Some(downloading(100 + UNKNOWN_SIZE_REPORT_STEP, None))
    );
    assert_eq!(tracker.installing().total, None);
}

#[test]
fn installing_and_done_carry_the_downloaded_size() {
    let mut tracker = ProgressTracker::default();
    tracker.chunk(4096, Some(4096));

    assert_eq!(
        tracker.installing(),
        UpdateProgress {
            phase: UpdatePhase::Installing,
            downloaded: 4096,
            total: Some(4096),
        }
    );
    assert_eq!(tracker.done().phase, UpdatePhase::Done);
    assert_eq!(tracker.done().downloaded, 4096);
}
