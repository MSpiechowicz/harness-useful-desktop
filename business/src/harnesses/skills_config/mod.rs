//! What Harness Skills keeps in a harness's config directory, shown read-only by the Claude Code
//! and OpenCode config views (harness-useful-skills README, "Claude Code model roles" and
//! "OpenCode model roles"). Each harness picks the settings root, the paths and the text:
//!
//! - Model roles: `harness-useful-skills/models.yml` below an install scope, YAML with a `modelRoles` map
//!   of role to selector. Listed only when the file exists. A project file that resolves outside
//!   the project is listed with an error and not read.
//! - Workflow: the repository master and the seven stage switches of
//!   `harness-useful-skills/workflow-settings/` below the settings root ([`workflow`]), as
//!   `key: "<value> (<source>)"` rows.
//!
//! Harness Skills before 0.4.0 used `useful-skills` for the folder. Both are read through
//! [`skills_data_path`]: the new path, else the old one when only it exists.

mod workflow;

use std::path::{Path, PathBuf};

pub(super) use self::workflow::MissingRoot;
use self::workflow::{read_settings, settings_dir};
use crate::ecosystem::skills_paths::skills_data_path;
use crate::model::{ConfigField, ConfigFormat, ConfigLayer, ConfigSection, ConfigSource, ConfigValue};
use crate::pi_family::{Document, SectionText, read_document, section_from, source};
use crate::security::canonical_within;

const OUTSIDE_PROJECT: &str = "The file resolves outside the project, so it is not read";

/// `harness-useful-skills/models.yml` below the install scope folder `scope`, or the old
/// `useful-skills/models.yml` when only that exists.
pub(super) fn models_path(scope: &Path) -> PathBuf {
    skills_data_path(scope, &["models.yml"])
}

/// The model roles file at `path`, or `None` when it does not exist. With `within`, a file that
/// resolves outside that project is an error section.
pub(super) fn models_section(
    text: SectionText<'_>,
    path: &Path,
    layer: ConfigLayer,
    within: Option<&Path>,
) -> Option<ConfigSection> {
    let file = source(path, layer, None, ConfigFormat::Yaml);
    if !file.exists {
        return None;
    }

    let outside = within.is_some_and(|project| canonical_within(project, path).is_err());

    let document = if outside {
        Document::Invalid(OUTSIDE_PROJECT.to_owned())
    } else {
        read_document(path, ConfigFormat::Yaml)
    };

    Some(section_from(text, file, &document))
}

/// Whether the workflow settings folder below `root` is there. A symlinked folder counts, so its
/// error is shown.
pub(super) fn settings_present(root: &Path) -> bool {
    std::fs::symlink_metadata(settings_dir(root)).is_ok()
}

/// The seven switches below `root` as `key: "<value> (<source>)"` rows (see
/// [`workflow::Setting::label`]), in the user layer.
pub(super) fn workflow_section(
    text: SectionText<'_>,
    root: &Path,
    missing: MissingRoot,
    project: Option<&Path>,
) -> ConfigSection {
    let entries = read_settings(root, missing, project)
        .into_iter()
        .map(|(key, setting)| ConfigField {
            key: key.to_owned(),
            value: ConfigValue::Text(setting.label()),
        })
        .collect();

    ConfigSection {
        id: text.id.to_owned(),
        title: text.title.to_owned(),
        description: Some(text.description.to_owned()),
        // The folder of the seven files. Its switches have a value even before any file is saved.
        source: ConfigSource {
            path: settings_dir(root).to_string_lossy().into_owned(),
            layer: ConfigLayer::User,
            profile: None,
            format: ConfigFormat::Json,
            exists: true,
        },
        entries,
        error: None,
    }
}
