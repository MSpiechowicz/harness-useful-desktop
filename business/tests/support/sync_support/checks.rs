//! Checks the "Share setup" export tests of several harnesses run on a collection: everything
//! selected, the excluded rows of a kind, nothing leaking, and an export that imports again.
//!
//! Each helper is used by some tests only, hence the `dead_code` allowance.
#![allow(dead_code)]

use harness_business::model::{SyncExcluded, SyncExportPreview, SyncItemKind};
use harness_business::sync::codec::{decode_input, to_json_pretty};
use harness_business::sync::schema::Bundle;
use harness_business::sync::{ExportCollection, build_bundle, export_code, export_preview};

use super::ids;

/// The bundle of every item `collection` offers.
pub fn bundle_of_everything(collection: &ExportCollection) -> Bundle {
    build_bundle(collection, &ids(&export_preview(collection)), None, None).unwrap()
}

/// The excluded rows of `kind`, in order.
pub fn excluded_of_kind(preview: &SyncExportPreview, kind: SyncItemKind) -> Vec<SyncExcluded> {
    preview
        .excluded
        .iter()
        .filter(|item| item.kind == kind)
        .cloned()
        .collect()
}

/// Asserts that no `secret` shows in the preview, the share file or the share string of
/// `collection`.
pub fn assert_nothing_shows(collection: &ExportCollection, secrets: &[&str]) {
    let preview = export_preview(collection);
    let bundle = bundle_of_everything(collection);
    let code = export_code(&bundle).unwrap().code;

    let texts = [
        ("preview", format!("{preview:?}")),
        ("file", to_json_pretty(&bundle).unwrap()),
        ("code", format!("{:?}", decode_input(&code).unwrap())),
    ];

    for (what, text) in texts {
        for secret in secrets {
            assert!(!text.contains(secret), "the {what} shows {secret}");
        }
    }
}

/// Asserts that the share string and the share file of everything `collection` offers import
/// again with no invalid and no unsupported item.
pub fn assert_imports_again(collection: &ExportCollection) {
    let bundle = bundle_of_everything(collection);

    for input in [export_code(&bundle).unwrap().code, to_json_pretty(&bundle).unwrap()] {
        let decoded = decode_input(&input).unwrap();

        assert_eq!(decoded.invalid_count(), 0);
        assert!(decoded.unsupported.is_empty());
    }
}
