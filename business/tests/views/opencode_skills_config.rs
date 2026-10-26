//! What Harness Skills keeps for OpenCode in the OpenCode config view, on OpenCode 1 and 2: the
//! model roles of each install scope's `harness-useful-skills/models.yml` and the workflow switches
//! of the user scope's `harness-useful-skills/workflow-settings/`, over the fixture home
//! (`opencode-home`, an install from before Harness Skills 0.4.0 with the folder `useful-skills`,
//! which is still read when only it exists) and synthetic homes.

use crate::common;

use std::path::{Path, PathBuf};

use common::{demo_project, env_at, opencode_home, set_mode, write_file};
use harness_business::env::Env;
use harness_business::harness::{Harness, OpenCodeGeneration};
use harness_business::harnesses::OpenCode;
use harness_business::model::{ConfigFormat, ConfigLayer, ConfigSection, ConfigValue, ConfigView};
use pretty_assertions::assert_eq;

use OpenCodeGeneration::{V1, V2};

const USER_MODELS: &str = "user.harnessSkillsModels";
const PROJECT_MODELS: &str = "project.harnessSkillsModels";
const WORKFLOW: &str = "user.harnessSkillsWorkflow";

fn config(generation: OpenCodeGeneration, env: &Env, project: Option<&Path>) -> ConfigView {
    OpenCode::with_generation(generation).config_view(env, project).unwrap()
}

fn find<'a>(view: &'a ConfigView, id: &str) -> Option<&'a ConfigSection> {
    view.sections.iter().find(|section| section.id == id)
}

/// The Harness Skills section ids of `view`, in order.
fn skills_ids(view: &ConfigView) -> Vec<&str> {
    view.sections
        .iter()
        .map(|section| section.id.as_str())
        .filter(|id| id.contains("harnessSkills"))
        .collect()
}

/// Each switch as `(key, label)`.
fn switches(view: &ConfigView) -> Vec<(String, String)> {
    find(view, WORKFLOW)
        .expect("a workflow section")
        .entries
        .iter()
        .map(|field| match &field.value {
            ConfigValue::Text(label) => (field.key.clone(), label.clone()),
            other => panic!("expected a label, got {other:?}"),
        })
        .collect()
}

fn label<'a>(switches: &'a [(String, String)], key: &str) -> &'a str {
    &switches
        .iter()
        .find(|(name, _)| name == key)
        .unwrap_or_else(|| panic!("switch {key} missing"))
        .1
}

/// The role `key` of a models section.
fn role<'a>(section: &'a ConfigSection, key: &str) -> &'a ConfigValue {
    let ConfigValue::Map(roles) = &section.entries[0].value else {
        panic!("modelRoles is a map");
    };

    &roles
        .iter()
        .find(|role| role.key == key)
        .unwrap_or_else(|| panic!("role {key} missing"))
        .value
}

fn text(value: &str) -> ConfigValue {
    ConfigValue::Text(value.to_owned())
}

/// A canonical temporary home with a project folder `work/app`.
fn machine() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let home = dunce::canonicalize(dir.path()).unwrap();
    let project = home.join("work/app");
    std::fs::create_dir_all(&project).unwrap();

    (dir, home, project)
}

#[test]
fn both_generations_show_the_user_and_project_roles_and_the_workflow() {
    let home = opencode_home();
    let project = demo_project();
    let env = env_at(&home);

    for generation in [V1, V2] {
        let view = config(generation, &env, Some(&project));
        assert_eq!(
            skills_ids(&view),
            [USER_MODELS, WORKFLOW, PROJECT_MODELS],
            "{generation:?}"
        );

        let user = find(&view, USER_MODELS).unwrap();
        assert_eq!(user.source.layer, ConfigLayer::User);
        assert_eq!(user.source.format, ConfigFormat::Yaml);
        assert_eq!(
            Path::new(&user.source.path),
            home.join(".config/opencode/useful-skills/models.yml")
        );
        assert_eq!(role(user, "default"), &text("ollama/llama3:8b"));
        assert_eq!(role(user, "security"), &text("inherit"));

        let project_roles = find(&view, PROJECT_MODELS).unwrap();
        assert_eq!(project_roles.source.layer, ConfigLayer::Project);
        assert_eq!(role(project_roles, "backend"), &text("openai/gpt-5:medium"));

        let shown = switches(&view);
        assert_eq!(label(&shown, "workflow"), "default (enabled)");
        assert_eq!(label(&shown, "plan"), "enabled (saved file)");
        assert_eq!(label(&shown, "review"), "disabled (saved file)");
        assert_eq!(label(&shown, "research"), "default (enabled)");
        assert!(
            !view.notices.iter().any(|notice| notice.message.contains("/plugin")),
            "OpenCode has no plugin option tier"
        );
    }
}

#[test]
fn the_user_scope_follows_xdg_and_never_opencode_config_dir() {
    let (_dir, home, _) = machine();
    let xdg = home.join("xdg");
    write_file(
        &xdg.join("opencode/useful-skills/models.yml"),
        "modelRoles:\n  plan: openai/gpt-5:high\n",
    );
    write_file(
        &home.join("team/useful-skills/models.yml"),
        "modelRoles:\n  plan: other/model\n",
    );

    let env = env_at(&home)
        .with_var("XDG_CONFIG_HOME", xdg.to_string_lossy())
        .with_var("OPENCODE_CONFIG_DIR", home.join("team").to_string_lossy());

    for generation in [V1, V2] {
        let view = config(generation, &env, None);
        let user = find(&view, USER_MODELS).expect("the XDG models file");

        assert_eq!(
            Path::new(&user.source.path),
            xdg.join("opencode/useful-skills/models.yml")
        );
        assert_eq!(role(user, "plan"), &text("openai/gpt-5:high"), "{generation:?}");
        assert_eq!(skills_ids(&view), [USER_MODELS]);
    }
}

#[test]
fn the_workflow_is_listed_only_when_installed_or_saved_and_nothing_is_created() {
    let (_dir, home, project) = machine();
    let env = env_at(&home);
    let config_dir = home.join(".config/opencode");

    assert!(skills_ids(&config(V1, &env, Some(&project))).is_empty());

    // A project install with no user config folder yet: every switch is a default.
    write_file(
        &project.join(".opencode/plugins/useful-skills/install-record.json"),
        r#"{"version":"0.3.8"}"#,
    );
    for generation in [V1, V2] {
        let shown = switches(&config(generation, &env, Some(&project)));
        assert!(
            shown.iter().all(|(_, label)| label == "default (enabled)"),
            "{generation:?}: {shown:?}"
        );
    }
    assert!(!config_dir.exists(), "nothing is created");
    std::fs::remove_dir_all(project.join(".opencode")).unwrap();

    write_file(
        &config_dir.join("plugins/useful-skills/install-record.json"),
        r#"{"version":"0.3.8"}"#,
    );
    let view = config(V2, &env, None);
    assert_eq!(skills_ids(&view), [WORKFLOW]);
    assert_eq!(label(&switches(&view), "workflow"), "not available (no project open)");
    assert!(!config_dir.join("useful-skills").exists(), "nothing is created");
}

#[test]
fn a_saved_master_of_the_project_checkout_is_read_on_both_generations() {
    use sha2::{Digest, Sha256};

    let (_dir, home, project) = machine();
    std::fs::create_dir(project.join(".git")).unwrap();

    let skills = home.join(".config/opencode/useful-skills");
    let repositories = skills.join("workflow-settings/repositories");
    // Harness Skills keys the checkout by its real path (Node's `realpath`), which has `\`
    // separators on Windows.
    let root = dunce::canonicalize(&project).unwrap();
    let local = repositories.join(format!("{:x}", Sha256::digest(root.to_string_lossy().as_bytes())));
    write_file(&local.join("workflow.json"), "false");

    for dir in [&skills, &skills.join("workflow-settings"), &repositories, &local] {
        set_mode(dir, 0o700);
    }

    for generation in [V1, V2] {
        let shown = switches(&config(generation, &env_at(&home), Some(&project)));
        assert_eq!(label(&shown, "workflow"), "disabled (saved file)", "{generation:?}");
    }
}

#[test]
fn project_roles_are_left_out_when_opencode_ignores_the_project() {
    let home = opencode_home();
    let project = demo_project();
    let env = env_at(&home).with_var("OPENCODE_DISABLE_PROJECT_CONFIG", "1");

    for generation in [V1, V2] {
        let view = config(generation, &env, Some(&project));
        assert_eq!(skills_ids(&view), [USER_MODELS, WORKFLOW], "{generation:?}");
    }
}

#[test]
fn an_empty_role_map_is_shown_as_written() {
    let (_dir, home, _) = machine();
    write_file(
        &home.join(".config/opencode/useful-skills/models.yml"),
        "# Installed default.\nmodelRoles:\n",
    );

    let view = config(V1, &env_at(&home), None);
    let user = find(&view, USER_MODELS).unwrap();

    assert_eq!(user.error, None);
    assert_eq!(user.entries.len(), 1);
    assert_eq!(user.entries[0].key, "modelRoles");
    assert_eq!(user.entries[0].value, ConfigValue::Null);
}

#[cfg(unix)]
#[test]
fn a_project_roles_file_that_resolves_outside_the_project_is_not_read() {
    let (_dir, home, project) = machine();
    let outside = home.join("elsewhere.yml");
    write_file(&outside, "modelRoles:\n  plan: secret/model\n");

    let link = project.join(".opencode/useful-skills/models.yml");
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    let view = config(V2, &env_at(&home), Some(&project));
    let section = find(&view, PROJECT_MODELS).expect("listed with an error");

    assert_eq!(
        section.error.as_deref(),
        Some("The file resolves outside the project, so it is not read")
    );
    assert!(section.entries.is_empty());
}

#[test]
fn the_roles_come_from_the_new_folder_the_old_one_or_the_new_one_when_both_exist() {
    let (_dir, home, project) = machine();
    let env = env_at(&home);
    let scopes = [home.join(".config/opencode"), project.join(".opencode")];

    for generation in [V1, V2] {
        for scope in &scopes {
            let old = scope.join("useful-skills/models.yml");
            let new = scope.join("harness-useful-skills/models.yml");
            let _ = std::fs::remove_dir_all(scope);

            write_file(&old, "modelRoles:\n  plan: old/model\n");
            let view = config(generation, &env, Some(&project));
            let section = find(
                &view,
                if scope == &scopes[0] {
                    USER_MODELS
                } else {
                    PROJECT_MODELS
                },
            )
            .unwrap();
            assert_eq!(Path::new(&section.source.path), old, "old only");
            assert_eq!(role(section, "plan"), &text("old/model"));

            write_file(&new, "modelRoles:\n  plan: new/model\n");
            let view = config(generation, &env, Some(&project));
            let section = find(
                &view,
                if scope == &scopes[0] {
                    USER_MODELS
                } else {
                    PROJECT_MODELS
                },
            )
            .unwrap();
            assert_eq!(Path::new(&section.source.path), new, "both");
            assert_eq!(role(section, "plan"), &text("new/model"));

            std::fs::remove_file(&old).unwrap();
            let view = config(generation, &env, Some(&project));
            let section = find(
                &view,
                if scope == &scopes[0] {
                    USER_MODELS
                } else {
                    PROJECT_MODELS
                },
            )
            .unwrap();
            assert_eq!(Path::new(&section.source.path), new, "new only");
            let _ = std::fs::remove_dir_all(scope);
        }
    }
}

#[test]
fn an_install_record_under_either_name_lists_the_workflow() {
    for folder in ["harness-useful-skills", "useful-skills"] {
        let (_dir, home, project) = machine();
        let env = env_at(&home);

        write_file(
            &project
                .join(".opencode/plugins")
                .join(folder)
                .join("install-record.json"),
            r#"{"version":"0.4.0"}"#,
        );
        assert_eq!(
            skills_ids(&config(V2, &env, Some(&project))),
            [WORKFLOW],
            "{folder}: project scope"
        );
        std::fs::remove_dir_all(project.join(".opencode")).unwrap();

        write_file(
            &home
                .join(".config/opencode/plugins")
                .join(folder)
                .join("install-record.json"),
            r#"{"version":"0.4.0"}"#,
        );
        assert_eq!(skills_ids(&config(V2, &env, None)), [WORKFLOW], "{folder}: user scope");
    }
}

#[test]
fn a_saved_master_in_either_folder_is_read() {
    use sha2::{Digest, Sha256};

    for folder in ["harness-useful-skills", "useful-skills"] {
        let (_dir, home, project) = machine();
        std::fs::create_dir(project.join(".git")).unwrap();

        let skills = home.join(".config/opencode").join(folder);
        let repositories = skills.join("workflow-settings/repositories");
        let root = dunce::canonicalize(&project).unwrap();
        let local = repositories.join(format!("{:x}", Sha256::digest(root.to_string_lossy().as_bytes())));
        write_file(&local.join("workflow.json"), "false");

        for dir in [&skills, &skills.join("workflow-settings"), &repositories, &local] {
            set_mode(dir, 0o700);
        }

        let shown = switches(&config(V1, &env_at(&home), Some(&project)));
        assert_eq!(label(&shown, "workflow"), "disabled (saved file)", "{folder}");
    }
}
