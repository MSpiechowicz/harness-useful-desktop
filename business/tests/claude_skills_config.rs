//! What Harness Skills keeps for Claude Code in the Claude Code config view: the model roles of
//! `useful-skills/models.yml` and the workflow switches of `useful-skills/workflow-settings/`, over
//! the fixture home (`claude-home`) and synthetic homes and checkouts.

mod common;

use std::path::{Path, PathBuf};

use common::{claude_home, demo_project, env_at, set_mode, write_file};
use harness_business::harness::Harness;
use harness_business::harnesses::Claude;
use harness_business::model::{ConfigFormat, ConfigLayer, ConfigSection, ConfigValue, ConfigView};
use pretty_assertions::assert_eq;
use sha2::{Digest, Sha256};

const MODELS: &str = "user.harnessSkillsModels";
const WORKFLOW: &str = "user.harnessSkillsWorkflow";

const OPTIONS_NOTICE: &str = "Harness Skills options set in Claude Code's /plugin configure flow apply when no saved \
                              file exists. They are not visible here.";

fn config(home: &Path, project: Option<&Path>) -> ConfigView {
    Claude.config_view(&env_at(home), project).unwrap()
}

fn find<'a>(view: &'a ConfigView, id: &str) -> Option<&'a ConfigSection> {
    view.sections.iter().find(|section| section.id == id)
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

/// The settings folder of `home`, its parent made private as the plugin requires.
fn settings_dir(home: &Path) -> PathBuf {
    let dir = home.join(".claude/useful-skills/workflow-settings");
    std::fs::create_dir_all(&dir).unwrap();
    set_mode(dir.parent().unwrap(), 0o700);
    set_mode(&dir, 0o700);

    dir
}

/// Writes the master of the checkout at `root` (canonical) with private folders.
fn write_master(home: &Path, root: &Path, content: &str) {
    let repositories = settings_dir(home).join("repositories");
    let local = repositories.join(format!("{:x}", Sha256::digest(root.to_string_lossy().as_bytes())));

    write_file(&local.join("workflow.json"), content);
    set_mode(&repositories, 0o700);
    set_mode(&local, 0o700);
}

/// A canonical temporary checkout (with a `.git` folder) and a folder inside it.
fn checkout() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let inner = root.join("packages/app");

    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(&inner).unwrap();

    (dir, root, inner)
}

#[test]
fn the_model_roles_reach_the_view_as_a_yaml_map() {
    let home = claude_home();
    let view = config(&home, None);
    let models = find(&view, MODELS).expect("a model roles section");

    assert_eq!(models.source.layer, ConfigLayer::User);
    assert_eq!(models.source.format, ConfigFormat::Yaml);
    assert!(models.source.path.ends_with("models.yml"));
    assert_eq!(models.entries.len(), 1);
    assert_eq!(models.entries[0].key, "modelRoles");

    let ConfigValue::Map(roles) = &models.entries[0].value else {
        panic!("modelRoles is a map");
    };
    let rows: Vec<(&str, &ConfigValue)> = roles.iter().map(|role| (role.key.as_str(), &role.value)).collect();

    assert_eq!(
        rows[0],
        ("research", &ConfigValue::Text("claude-haiku-5-5:high".to_owned()))
    );
    assert_eq!(rows[7], ("default", &ConfigValue::Text("inherit".to_owned())));
}

#[test]
fn an_unparsable_models_file_is_an_error_section_and_a_missing_one_none() {
    let home = tempfile::tempdir().unwrap();
    write_file(
        &home.path().join(".claude/useful-skills/models.yml"),
        "modelRoles: [oops",
    );

    let view = config(home.path(), None);
    let error = find(&view, MODELS).and_then(|section| section.error.as_deref());

    assert!(
        error.is_some_and(|error| error.starts_with("Invalid YAML")),
        "{error:?}"
    );

    let empty = tempfile::tempdir().unwrap();
    assert!(config(empty.path(), None).sections.is_empty());
}

#[test]
fn the_fixture_switches_show_saved_files_and_defaults() {
    let home = claude_home();
    let project = demo_project();
    let view = config(&home, Some(&project));
    let shown = switches(&view);

    let keys: Vec<&str> = shown.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "workflow",
            "research",
            "plan",
            "review",
            "security-review",
            "backend-memory",
            "graphify-memory"
        ]
    );
    assert_eq!(label(&shown, "workflow"), "default (enabled)");
    assert_eq!(label(&shown, "research"), "enabled (saved file)");
    assert_eq!(label(&shown, "plan"), "disabled (saved file)");
    assert_eq!(label(&shown, "review"), "enabled (saved file)");
    assert_eq!(label(&shown, "graphify-memory"), "default (enabled)");
    assert!(view.notices.iter().any(|notice| notice.message == OPTIONS_NOTICE));

    let section = find(&view, WORKFLOW).unwrap();
    assert!(section.source.exists);
    assert!(section.source.path.ends_with("workflow-settings"));
}

#[test]
fn the_master_follows_the_checkout_root_and_is_not_available_without_a_project() {
    let home = tempfile::tempdir().unwrap();
    let (_checkout, root, inner) = checkout();
    write_master(home.path(), &root, "false");

    let from_inside = switches(&config(home.path(), Some(&inner)));
    assert_eq!(label(&from_inside, "workflow"), "disabled (saved file)");
    assert_eq!(label(&from_inside, "research"), "default (enabled)");

    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::create_dir(elsewhere.path().join(".git")).unwrap();
    assert_eq!(
        label(&switches(&config(home.path(), Some(elsewhere.path()))), "workflow"),
        "default (enabled)"
    );

    assert_eq!(
        label(&switches(&config(home.path(), None)), "workflow"),
        "not available (no project open)"
    );
}

#[test]
fn a_gitdir_file_marks_the_checkout_of_a_worktree() {
    let home = tempfile::tempdir().unwrap();
    let (_main, main_root, _) = checkout();
    let (_tree, tree_root, inner) = checkout();

    let admin = main_root.join(".git/worktrees/feature");
    std::fs::create_dir_all(&admin).unwrap();
    std::fs::remove_dir(tree_root.join(".git")).unwrap();
    write_file(&tree_root.join(".git"), &format!("gitdir: {}\n", admin.display()));
    write_master(home.path(), &tree_root, "false");

    assert_eq!(
        label(&switches(&config(home.path(), Some(&inner))), "workflow"),
        "disabled (saved file)"
    );

    write_file(&tree_root.join(".git"), "not a marker\n");
    assert_eq!(
        label(&switches(&config(home.path(), Some(&inner))), "workflow"),
        "error: Cannot safely resolve workspace Git marker."
    );
}

#[test]
fn an_invalid_stage_file_is_an_error_for_its_key_only() {
    let home = tempfile::tempdir().unwrap();
    let dir = settings_dir(home.path());
    write_file(&dir.join("plan.json"), "\"yes\"");
    write_file(&dir.join("review.json"), &"true ".repeat(1000));
    write_file(&dir.join("research.json"), "false");

    let shown = switches(&config(home.path(), None));

    assert_eq!(
        label(&shown, "plan"),
        "error: Workflow setting must contain a JSON boolean."
    );
    assert_eq!(
        label(&shown, "review"),
        "error: Workflow setting exceeds the size limit."
    );
    assert_eq!(label(&shown, "research"), "disabled (saved file)");
    assert_eq!(label(&shown, "security-review"), "default (enabled)");
}

#[cfg(unix)]
#[test]
fn a_symlinked_stage_or_a_shared_folder_is_an_error() {
    let home = tempfile::tempdir().unwrap();
    let dir = settings_dir(home.path());
    let elsewhere = home.path().join("elsewhere.json");
    write_file(&elsewhere, "false");
    std::os::unix::fs::symlink(&elsewhere, dir.join("plan.json")).unwrap();

    let shown = switches(&config(home.path(), None));
    assert_eq!(
        label(&shown, "plan"),
        "error: Workflow setting must be a non-symlink regular file."
    );

    set_mode(&dir, 0o755);
    let shown = switches(&config(home.path(), None));
    assert!(
        shown
            .iter()
            .all(|(_, label)| label == "error: Managed workflow ancestor must be private."),
        "{shown:?}"
    );
}

#[test]
fn the_workflow_is_listed_only_when_harness_skills_is_installed_or_saved() {
    let home = tempfile::tempdir().unwrap();
    let registry = home.path().join(".claude/plugins/installed_plugins.json");

    write_file(
        &registry,
        r#"{ "version": 2, "plugins": { "other@market": [{ "scope": "user" }] } }"#,
    );
    let view = config(home.path(), None);
    assert!(find(&view, WORKFLOW).is_none());
    assert!(!view.notices.iter().any(|notice| notice.message == OPTIONS_NOTICE));

    write_file(
        &registry,
        r#"{ "version": 2, "plugins": { "useful-skills@useful-skills-local": [{ "scope": "user" }] } }"#,
    );
    let view = config(home.path(), None);
    assert!(find(&view, MODELS).is_none());
    assert!(
        switches(&view)
            .iter()
            .skip(1)
            .all(|(_, label)| label == "default (enabled)")
    );
    assert!(
        !home.path().join(".claude/useful-skills").exists(),
        "nothing is created"
    );
}
