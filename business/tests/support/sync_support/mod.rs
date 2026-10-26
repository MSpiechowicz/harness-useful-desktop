//! A temporary home for the "Share setup" export tests, and harness summaries.

mod checks;
mod harnesses;
pub mod servers;

pub use checks::{assert_imports_again, assert_nothing_shows, bundle_of_everything, excluded_of_kind};
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
        self.collect_from(&HarnessRegistry::with_defaults(), installed)
    }
}

/// Helpers some test crates use only, hence the `dead_code` allowance.
#[allow(dead_code)]
impl Home {
    /// [`Self::collect`] with the harnesses of `registry`.
    pub fn collect_from(&self, registry: &HarnessRegistry, installed: &[HarnessId]) -> ExportCollection {
        collect_export(&self.env, registry, &summaries(installed))
    }

    /// Sets the environment variable `name` of this home to `value`.
    pub fn set_var(&mut self, name: &str, value: &str) {
        self.env = self.env.clone().with_var(name, value);
    }

    /// The absolute path of `relative` in this home.
    pub fn path(&self, relative: &str) -> std::path::PathBuf {
        self.dir.path().join(relative)
    }
}

pub fn ids(preview: &SyncExportPreview) -> Vec<String> {
    preview
        .sections
        .iter()
        .flat_map(|section| section.items.iter().map(|item| item.id.clone()))
        .collect()
}
