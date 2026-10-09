//! The in-app updater's rules that do not depend on Tauri: where updates come from, whether this
//! build can verify them, what the UI is told about a release, and how download progress is
//! reported.
//!
//! The shell checks [`UPDATE_ENDPOINT`] with `tauri-plugin-updater`, which verifies every download
//! against the public key built into the app before installing it. A build without a usable key
//! (see [`configured_pubkey`]) never contacts the endpoint.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use minisign_verify::PublicKey;

use crate::model::{MAX_UPDATE_NOTES_BYTES, UpdateInfo, UpdatePhase, UpdateProgress};

/// The release manifest written by the release workflow (`tauri-action`, `includeUpdaterJson`).
/// The only URL the updater requests; the downloads it lists must be HTTPS as well.
pub const UPDATE_ENDPOINT: &str =
    "https://github.com/MSpiechowicz/harness-useful-desktop/releases/latest/download/latest.json";

/// What `update_check` and `update_install` report when the build has no usable public key.
pub const UPDATES_NOT_CONFIGURED: &str = "updates are not configured for this build";

/// `plugins.updater.pubkey` in `tauri.conf.json` until the maintainer adds the real public key
/// (`docs/release-and-updater-keys.md`). It is not a key, so builds carrying it skip updates.
pub const PUBKEY_PLACEHOLDER: &str =
    "NOT_CONFIGURED: replace with the content of the updater .key.pub file (see docs/release-and-updater-keys.md)";

/// The configured updater public key when it is one the updater can verify signatures with:
/// the base64-encoded content of a minisign `.pub` file, as `tauri signer generate` writes it.
///
/// `None` for a missing or empty value, the [`PUBKEY_PLACEHOLDER`], or anything else that does not
/// decode to a minisign public key. The updater decodes the key the same way.
pub fn configured_pubkey(value: Option<&str>) -> Option<&str> {
    let value = value?;

    let decoded = STANDARD.decode(value).ok()?;
    let text = std::str::from_utf8(&decoded).ok()?;

    PublicKey::decode(text).ok().map(|_| value)
}

impl UpdateInfo {
    /// The info shown for an announced release. Empty notes become `None`; notes longer than
    /// [`MAX_UPDATE_NOTES_BYTES`] are cut at a character boundary.
    pub fn announced(version: String, current_version: String, date: Option<String>, notes: Option<String>) -> Self {
        let notes = notes
            .filter(|notes| !notes.trim().is_empty())
            .map(|notes| cut_to(notes, MAX_UPDATE_NOTES_BYTES));

        Self {
            version,
            current_version,
            date,
            notes,
        }
    }
}

fn cut_to(mut text: String, max: usize) -> String {
    if text.len() <= max {
        return text;
    }

    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }

    text.truncate(end);
    text
}

/// How many bytes pass between two download reports when the size is unknown.
pub const UNKNOWN_SIZE_REPORT_STEP: u64 = 512 * 1024;

/// Turns the updater's per-chunk download callbacks into [`UpdateProgress`] reports.
///
/// Downloads arrive in many small chunks, so [`chunk`](Self::chunk) reports at most once per
/// percent of a known size (or every [`UNKNOWN_SIZE_REPORT_STEP`] bytes otherwise), plus the first
/// chunk and the last one of a known size.
#[derive(Debug, Default)]
pub struct ProgressTracker {
    downloaded: u64,
    total: Option<u64>,
    reported_at: Option<u64>,
}

impl ProgressTracker {
    /// Records a downloaded chunk of `len` bytes; `total` is the size the server announced. Returns
    /// the report to send, if one is due.
    ///
    /// A size the download has outgrown is no longer trusted: `total` is `None` from then on.
    pub fn chunk(&mut self, len: usize, total: Option<u64>) -> Option<UpdateProgress> {
        let len = u64::try_from(len).unwrap_or(u64::MAX);
        self.downloaded = self.downloaded.saturating_add(len);
        self.total = total.filter(|total| *total >= self.downloaded);

        if !self.report_due() {
            return None;
        }

        self.reported_at = Some(self.downloaded);
        Some(self.progress(UpdatePhase::Downloading))
    }

    fn report_due(&self) -> bool {
        let Some(reported_at) = self.reported_at else {
            return true;
        };

        if self.total == Some(self.downloaded) {
            return true;
        }

        let step = match self.total {
            Some(total) => (total / 100).max(1),
            None => UNKNOWN_SIZE_REPORT_STEP,
        };

        self.downloaded - reported_at >= step
    }

    /// The report sent once the download is verified and installing starts.
    pub fn installing(&self) -> UpdateProgress {
        self.progress(UpdatePhase::Installing)
    }

    /// The report sent once the update is installed.
    pub fn done(&self) -> UpdateProgress {
        self.progress(UpdatePhase::Done)
    }

    fn progress(&self, phase: UpdatePhase) -> UpdateProgress {
        UpdateProgress {
            phase,
            downloaded: self.downloaded,
            total: self.total,
        }
    }
}
