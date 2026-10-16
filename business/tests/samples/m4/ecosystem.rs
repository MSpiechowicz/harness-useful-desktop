//! `ecosystem/status.json`: the dashboard, the memory server and the skills installed, the sidebar
//! not yet (Linux).
//! Names, links, install steps and notes come from the real catalog; only the detection is
//! synthetic.

use harness_business::ecosystem::catalog_item;
use harness_business::env::OsKind;
use harness_business::model::{EcosystemId, EcosystemItem, EcosystemStatus};
use serde_json::Value;

use super::to_json;

fn installed(id: EcosystemId, version: &str, via: &str) -> EcosystemItem {
    EcosystemItem {
        installed: true,
        version: Some(version.to_owned()),
        via: vec![via.to_owned()],
        ..catalog_item(id, OsKind::Linux)
    }
}

/// The `ecosystem_status` result.
pub fn status() -> Value {
    to_json(EcosystemStatus {
        items: vec![
            installed(
                EcosystemId::Dashboard,
                "1.22.1",
                "binary ~/.local/bin/harness-dashboard",
            ),
            installed(EcosystemId::Memory, "1.0.0", "binary ~/.local/bin/harness-memory"),
            installed(
                EcosystemId::Skills,
                "0.2.36",
                "OMP plugin harness-useful-skills@harness-useful-skills v0.2.36",
            ),
            catalog_item(EcosystemId::Sidebar, OsKind::Linux),
        ],
    })
}
