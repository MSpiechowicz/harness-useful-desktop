//! Helpers of the crafted Claude mapper tests: the hints of mapped output and frames they build.
#![allow(dead_code)]

use harness_business::claude_stream::mapper::{Hint, Mapped};
use serde_json::{Value, json};

/// The hints of `mapped`, in order.
pub fn hints(mapped: &[Mapped]) -> Vec<Hint> {
    mapped
        .iter()
        .filter_map(|mapped| match mapped {
            Mapped::Hint(hint) => Some(hint.clone()),
            Mapped::Event(_) => None,
        })
        .collect()
}

/// `system/status requesting` serving prompt `uuid`.
pub fn requesting(uuid: &str) -> Value {
    json!({ "type": "system", "subtype": "status", "status": "requesting", "user_message_uuids": [uuid] })
}
