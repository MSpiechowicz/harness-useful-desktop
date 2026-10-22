//! What Harness Skills keeps for Claude Code in the config directory (harness-useful-skills README,
//! "Claude Code" and "Claude Code model roles"), shown read-only in the user layer:
//!
//! - Model roles: `useful-skills/models.yml`, YAML with a `modelRoles` map of role to selector
//!   (`<model>[:<effort>]`). Listed only when the file exists.
//! - Workflow: the repository master and the six stage switches ([`workflow`]). Listed when the
//!   Harness Skills plugin (`useful-skills@<marketplace>`) is in `installed_plugins.json` or the
//!   settings folder exists. The plugin's `/plugin` options reach only Claude Code's hooks, so a
//!   notice says they apply where no saved file exists and are not shown.

use std::path::Path;

use super::super::paths::config_dir;
use super::super::setup::plugin_records;
use super::workflow::{read_settings, settings_dir};
use crate::env::Env;
use crate::harnesses::plugin_registry::split_id;
use crate::model::{
    ConfigField, ConfigFormat, ConfigLayer, ConfigSection, ConfigSource, ConfigValue, Notice, NoticeLevel,
};
use crate::pi_family::{SectionText, optional_section, source};

/// The plugin's name in Claude Code's registry, whatever marketplace it came from.
const PLUGIN_NAME: &str = "useful-skills";

const MODELS_TEXT: SectionText<'static> = SectionText {
    id: "user.harnessSkillsModels",
    title: "Harness Skills model roles",
    description: "The model and effort of each Harness Skills role. Claude Code picks up changes in its next session.",
};

const WORKFLOW_ID: &str = "user.harnessSkillsWorkflow";

const WORKFLOW_TITLE: &str = "Harness Skills workflow";

const WORKFLOW_DESCRIPTION: &str = "The Harness Skills workflow for the Git checkout of this project and its six stages for every project. \
                                    Change them with /useful-skills in Claude Code.";

const OPTIONS_NOTICE: &str = "Harness Skills options set in Claude Code's /plugin configure flow apply when no saved file exists. \
                              They are not visible here.";

/// The Harness Skills sections, and the notice about `/plugin` options when the workflow is shown.
pub(super) fn sections(env: &Env, project: Option<&Path>) -> (Vec<ConfigSection>, Option<Notice>) {
    let config = config_dir(env);
    let models = config.join("useful-skills").join("models.yml");

    let mut sections: Vec<ConfigSection> = optional_section(
        MODELS_TEXT,
        source(&models, ConfigLayer::User, None, ConfigFormat::Yaml),
    )
    .into_iter()
    .collect();

    let directory = settings_dir(&config);
    // A symlinked folder counts as present, so its error is shown.
    let present = std::fs::symlink_metadata(&directory).is_ok();
    if !present && !is_installed(env) {
        return (sections, None);
    }

    sections.push(workflow_section(&config, &directory, project));

    let notice = Notice {
        level: NoticeLevel::Info,
        message: OPTIONS_NOTICE.to_owned(),
    };

    (sections, Some(notice))
}

/// The seven switches as `key: "<value> (<source>)"` rows (see [`super::workflow::Setting::label`]).
fn workflow_section(config: &Path, directory: &Path, project: Option<&Path>) -> ConfigSection {
    let entries = read_settings(config, project)
        .into_iter()
        .map(|(key, setting)| ConfigField {
            key: key.to_owned(),
            value: ConfigValue::Text(setting.label()),
        })
        .collect();

    ConfigSection {
        id: WORKFLOW_ID.to_owned(),
        title: WORKFLOW_TITLE.to_owned(),
        description: Some(WORKFLOW_DESCRIPTION.to_owned()),
        // The folder of the seven files. Its switches have a value even before any file is saved.
        source: ConfigSource {
            path: directory.to_string_lossy().into_owned(),
            layer: ConfigLayer::User,
            profile: None,
            format: ConfigFormat::Json,
            exists: true,
        },
        entries,
        error: None,
    }
}

/// Whether Claude Code's plugin registry lists the Harness Skills plugin in any scope.
fn is_installed(env: &Env) -> bool {
    plugin_records(env)
        .iter()
        .any(|plugin| split_id(&plugin.id).0 == PLUGIN_NAME)
}
