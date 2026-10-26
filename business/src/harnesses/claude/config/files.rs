//! The settings files Claude Code reads, in the order the view shows them: managed settings
//! (`managed-settings.json`, then its `managed-settings.d/*.json` drop-ins by name), the user's
//! `settings.json`, the project's `.claude/settings.json` and `.claude/settings.local.json`. Only
//! files that exist are listed. A project file that resolves outside the project (a symlink to
//! the user's files) is listed with an error and nothing read from it.

use std::path::{Path, PathBuf};

use super::super::paths::{PROJECT_DIR_NAME, config_dir};
use crate::env::{Env, OsKind};
use crate::model::{ConfigFormat, ConfigLayer};
use crate::pi_family::{Document, list_dir, read_document};
use crate::security::canonical_within;

/// The managed settings file in the managed directory.
const MANAGED_FILE: &str = "managed-settings.json";

/// The managed drop-in folder next to it.
const DROP_IN_DIR: &str = "managed-settings.d";

/// Most drop-ins listed; real setups have a handful.
const MAX_DROP_INS: usize = 64;

/// The user's and the project's settings file name.
const SETTINGS_FILE: &str = "settings.json";

/// The project's own settings file name, kept out of version control.
const LOCAL_SETTINGS_FILE: &str = "settings.local.json";

/// One settings file of the view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::harnesses::claude) struct SettingsFile {
    /// Section id.
    pub id: String,
    /// Section title.
    pub title: String,
    /// What the file controls.
    pub description: &'static str,
    /// Where it is.
    pub path: PathBuf,
    /// The layer it is shown in. Managed settings have no layer of their own and are shown with the
    /// user's, above the user's own file.
    pub layer: ConfigLayer,
    /// The project a project file must resolve inside to be read.
    pub within: Option<PathBuf>,
}

impl SettingsFile {
    /// The file's document; an error when it resolves outside [`SettingsFile::within`].
    pub(in crate::harnesses::claude) fn read(&self) -> Document {
        let outside = self
            .within
            .as_deref()
            .is_some_and(|project| self.path.exists() && canonical_within(project, &self.path).is_err());

        if outside {
            return Document::Invalid("The file resolves outside the project, so it is not read".to_owned());
        }

        read_document(&self.path, ConfigFormat::Json)
    }
}

/// The managed settings files that exist.
pub(in crate::harnesses::claude) fn managed_files(env: &Env) -> Vec<SettingsFile> {
    let dir = managed_dir(env);
    let mut files = Vec::new();

    let base = dir.join(MANAGED_FILE);
    if base.is_file() {
        files.push(SettingsFile {
            id: "managed.settings".to_owned(),
            title: "Managed settings".to_owned(),
            description: "Settings your organization manages. They override every other file.",
            path: base,
            layer: ConfigLayer::User,
            within: None,
        });
    }

    let drop_ins = list_dir(&dir.join(DROP_IN_DIR))
        .into_iter()
        .filter(|entry| entry.is_file && entry.name.ends_with(".json") && !entry.name.starts_with('.'))
        .take(MAX_DROP_INS);

    files.extend(drop_ins.map(|entry| SettingsFile {
        id: format!("managed.dropIn:{}", entry.name),
        title: format!("Managed settings ({})", entry.name),
        description: "A managed drop-in file. Claude Code applies the drop-ins after managed-settings.json, in name order.",
        path: entry.path,
        layer: ConfigLayer::User,
        within: None,
    }));

    files
}

/// The user's settings and, with a project, the project's shared and local settings, when they
/// exist. A project file that is the user's file (the project is the home directory) is listed once.
pub(in crate::harnesses::claude) fn settings_files(env: &Env, project: Option<&Path>) -> Vec<SettingsFile> {
    let user = config_dir(env).join(SETTINGS_FILE);
    let mut files = vec![SettingsFile {
        id: "user.settings".to_owned(),
        title: "Settings".to_owned(),
        description: "Global Claude Code settings: model, permissions, hooks, environment and plugins.",
        path: user.clone(),
        layer: ConfigLayer::User,
        within: None,
    }];

    if let Some(project) = project {
        let dir = project.join(PROJECT_DIR_NAME);

        files.push(SettingsFile {
            id: "project.settings".to_owned(),
            title: "Project settings".to_owned(),
            description: "Settings shared with everyone working on this project, usually committed.",
            path: dir.join(SETTINGS_FILE),
            layer: ConfigLayer::Project,
            within: Some(project.to_path_buf()),
        });
        files.push(SettingsFile {
            id: "project.localSettings".to_owned(),
            title: "Local project settings".to_owned(),
            description: "Your own settings for this project, not committed. They override the project settings.",
            path: dir.join(LOCAL_SETTINGS_FILE),
            layer: ConfigLayer::Project,
            within: Some(project.to_path_buf()),
        });
    }

    files.retain(|file| file.path.is_file() && (file.layer == ConfigLayer::User || file.path != user));
    files
}

/// The managed settings directory of the OS: `/etc/claude-code`, `/Library/Application
/// Support/ClaudeCode` or `%ProgramFiles%\ClaudeCode`.
fn managed_dir(env: &Env) -> PathBuf {
    match env.os {
        OsKind::Linux => PathBuf::from("/etc/claude-code"),
        OsKind::MacOs => PathBuf::from("/Library/Application Support/ClaudeCode"),
        OsKind::Windows => PathBuf::from(env.var("ProgramFiles").unwrap_or("C:\\Program Files")).join("ClaudeCode"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_managed_directory_follows_the_os() {
        let env = Env::for_tests("/home/u");

        assert_eq!(
            managed_dir(&env.clone().with_os(OsKind::Linux)),
            PathBuf::from("/etc/claude-code")
        );
        assert_eq!(
            managed_dir(&env.clone().with_os(OsKind::MacOs)),
            PathBuf::from("/Library/Application Support/ClaudeCode")
        );
        assert_eq!(
            managed_dir(&env.clone().with_os(OsKind::Windows)),
            PathBuf::from("C:\\Program Files").join("ClaudeCode")
        );
        assert_eq!(
            managed_dir(&env.with_os(OsKind::Windows).with_var("ProgramFiles", "D:\\Apps")),
            PathBuf::from("D:\\Apps").join("ClaudeCode")
        );
    }
}
