//! What Harness Skills keeps for Claude Code in the config directory (harness-useful-skills README,
//! "Claude Code" and "Claude Code model roles"), shown read-only in the user layer through
//! [`crate::harnesses::skills_config`]:
//!
//! - Model roles: `harness-useful-skills/models.yml` (`useful-skills/models.yml` before Harness
//!   Skills 0.4.0, used when only it exists), YAML with a `modelRoles` map of role to selector
//!   (`<model>[:<effort>]`). Listed only when the file exists.
//! - Workflow: the repository master and the six stage switches. Listed when the Harness Skills
//!   plugin (`harness-useful-skills@<marketplace>`, or `useful-skills@<marketplace>` before 0.4.0) is in
//!   `installed_plugins.json` or the settings folder exists. The plugin's `/plugin` options reach only Claude Code's hooks, so a notice says they
//!   apply where no saved file exists and are not shown.

use std::path::Path;

use super::super::paths::config_dir;
use super::super::setup::plugin_records;
use crate::ecosystem::catalog::{LEGACY_SKILLS_PLUGIN, SKILLS_PLUGIN};
use crate::env::Env;
use crate::harnesses::plugin_registry::split_id;
use crate::harnesses::skills_config::{MissingRoot, models_path, models_section, settings_present, workflow_section};
use crate::model::{ConfigLayer, ConfigSection, Notice, NoticeLevel};
use crate::pi_family::SectionText;

/// The plugin's names in Claude Code's registry (new, then before 0.4.0), whatever marketplace it
/// came from.
const PLUGIN_NAMES: [&str; 2] = [SKILLS_PLUGIN, LEGACY_SKILLS_PLUGIN];

const MODELS_TEXT: SectionText<'static> = SectionText {
    id: "user.harnessSkillsModels",
    title: "Harness Skills model roles",
    description: "The model and effort of each Harness Skills role. Claude Code picks up changes in its next session.",
};

const WORKFLOW_TEXT: SectionText<'static> = SectionText {
    id: "user.harnessSkillsWorkflow",
    title: "Harness Skills workflow",
    description: "The Harness Skills workflow for the Git checkout of this project and its six stages for every project. \
                  Change them with /useful-skills in Claude Code.",
};

const OPTIONS_NOTICE: &str = "Harness Skills options set in Claude Code's /plugin configure flow apply when no saved file exists. \
                              They are not visible here.";

/// The Harness Skills sections, and the notice about `/plugin` options when the workflow is shown.
pub(super) fn sections(env: &Env, project: Option<&Path>) -> (Vec<ConfigSection>, Option<Notice>) {
    let config = config_dir(env);

    let mut sections: Vec<ConfigSection> = models_section(MODELS_TEXT, &models_path(&config), ConfigLayer::User, None)
        .into_iter()
        .collect();

    if !settings_present(&config) && !is_installed(env) {
        return (sections, None);
    }

    sections.push(workflow_section(WORKFLOW_TEXT, &config, MissingRoot::Error, project));

    let notice = Notice {
        level: NoticeLevel::Info,
        message: OPTIONS_NOTICE.to_owned(),
    };

    (sections, Some(notice))
}

/// Whether Claude Code's plugin registry lists the Harness Skills plugin in any scope.
fn is_installed(env: &Env) -> bool {
    plugin_records(env)
        .iter()
        .any(|plugin| PLUGIN_NAMES.contains(&split_id(&plugin.id).0))
}
