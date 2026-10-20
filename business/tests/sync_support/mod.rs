//! A temporary home for the "Share setup" export tests, and harness summaries.

mod harnesses;
pub mod servers;

use harness_business::env::Env;
use harness_business::harness::HarnessRegistry;
use harness_business::model::{HarnessId, SyncExportPreview};
use harness_business::sync::{ExportCollection, collect_export};
pub use harnesses::{BOTH, summaries};
use serde_json::Value;
use tempfile::TempDir;

pub struct Home {
    dir: TempDir,
    env: Env,
}

impl Home {
    pub fn empty() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::for_tests(dir.path());
        Self { dir, env }
    }

    pub fn write(&self, relative: &str, value: &Value) {
        self.write_text(relative, &serde_json::to_string_pretty(value).unwrap());
    }

    pub fn write_text(&self, relative: &str, text: &str) {
        let path = self.dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// The export of this home with `installed` detected as installed.
    pub fn collect(&self, installed: &[HarnessId]) -> ExportCollection {
        collect_export(&self.env, &HarnessRegistry::with_defaults(), &summaries(installed))
    }
}

pub fn ids(preview: &SyncExportPreview) -> Vec<String> {
    preview
        .sections
        .iter()
        .flat_map(|section| section.items.iter().map(|item| item.id.clone()))
        .collect()
}
