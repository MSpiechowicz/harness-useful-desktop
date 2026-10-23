//! What Harness Skills keeps for OpenCode (harness-useful-skills README, "OpenCode" and "OpenCode
//! model roles", `opencode/models.js` and `opencode/settings.js`), shown read-only through
//! [`crate::harnesses::skills_config`] on OpenCode 1 and 2 alike:
//!
//! - Model roles: `useful-skills/models.yml` in each install scope, YAML with a `modelRoles` map of
//!   role to selector (`<provider>/<model>[:<variant>]` or `inherit`). The user scope is
//!   `$XDG_CONFIG_HOME/opencode` (else `~/.config/opencode`), never `OPENCODE_CONFIG_DIR`, which is
//!   no install target. The project scope is `<project>/.opencode`, left out when OpenCode ignores
//!   the project's settings. Each is listed in its layer, only when the file exists.
//! - Workflow: the repository master and the six stage switches in
//!   `useful-skills/workflow-settings/` of the user scope, for both install scopes. Listed when
//!   an install record of either scope or the settings folder exists. A user scope folder that does
//!   not exist yet holds no saved settings. The plugin has no option tier, so there is no notice.

use std::fs;
use std::path::Path;

use super::layers::project_config_enabled;
use crate::ecosystem::installer::Installed;
use crate::ecosystem::skills_record::skills_installed;
use crate::env::Env;
use crate::harness::{OpenCodeGeneration, SkillsHost};
use crate::harnesses::opencode::{PROJECT_DIR_NAME, config_dir};
use crate::harnesses::skills_config::{MissingRoot, models_path, models_section, settings_present, workflow_section};
use crate::model::{ConfigLayer, ConfigSection};
use crate::pi_family::SectionText;

const USER_MODELS_TEXT: SectionText<'static> = SectionText {
    id: "user.harnessSkillsModels",
    title: "Harness Skills model roles",
    description: "The model and variant of each Harness Skills role. OpenCode picks up changes in its next session.",
};

const PROJECT_MODELS_TEXT: SectionText<'static> = SectionText {
    id: "project.harnessSkillsModels",
    title: "Project Harness Skills model roles",
    description: "The model and variant of each Harness Skills role installed in this project. \
                  OpenCode picks up changes in its next session.",
};

const WORKFLOW_TEXT: SectionText<'static> = SectionText {
    id: "user.harnessSkillsWorkflow",
    title: "Harness Skills workflow",
    description: "The Harness Skills workflow for the Git checkout of this project and its six stages for every project. \
                  Change them with /useful-skills in OpenCode.",
};

/// The Harness Skills sections: the user's model roles and the workflow, then the project's model
/// roles.
pub(super) fn sections(env: &Env, project: Option<&Path>, generation: OpenCodeGeneration) -> Vec<ConfigSection> {
    let config = config_dir(env);
    let project_scope = project
        .filter(|_| project_config_enabled(env, generation))
        .map(|project| (project, project.join(PROJECT_DIR_NAME)));

    let mut sections: Vec<ConfigSection> =
        models_section(USER_MODELS_TEXT, &models_path(&config), ConfigLayer::User, None)
            .into_iter()
            .collect();

    let installed_in_project = project_scope.as_ref().is_some_and(|(_, scope)| has_record(scope));

    if settings_present(&config)
        || installed_in_project
        || skills_installed(env, SkillsHost::OpenCode) != Installed::Missing
    {
        sections.push(workflow_section(WORKFLOW_TEXT, &config, MissingRoot::Defaults, project));
    }

    if let Some((project, scope)) = project_scope {
        sections.extend(models_section(
            PROJECT_MODELS_TEXT,
            &models_path(&scope),
            ConfigLayer::Project,
            Some(project),
        ));
    }

    sections
}

/// Whether the install scope folder `scope` holds an install record, usable or not (the user
/// scope's is [`skills_installed`]).
fn has_record(scope: &Path) -> bool {
    let record = scope.join("plugins").join("useful-skills").join("install-record.json");

    fs::symlink_metadata(record).is_ok()
}
