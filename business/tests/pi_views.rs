//! PI config and extensions views over the synthetic fixture home.

mod common;

use std::path::Path;

use common::{demo_project, env_at, exported, pi_home};
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harnesses::Pi;
use harness_business::model::{ConfigLayer, ConfigValue, ConfigView, ExtensionScope, ExtensionsView, SecretKind};
use pretty_assertions::assert_eq;

const TRUST_NOTICE: &str = "Project resources load only for trusted projects (Pi project trust).";

fn config(env: &Env, project: Option<&Path>) -> ConfigView {
    Pi.config_view(env, project).unwrap()
}

fn extensions(env: &Env, project: Option<&Path>) -> ExtensionsView {
    Pi.extensions_view(env, project).unwrap()
}

#[test]
fn config_view_snapshots() {
    let home = pi_home();
    let project = demo_project();
    let env = env_at(&home);

    insta::assert_json_snapshot!("pi_config_user", exported(config(&env, None), &home, &project));
    insta::assert_json_snapshot!(
        "pi_config_project",
        exported(config(&env, Some(&project)), &home, &project)
    );
}

#[test]
fn extensions_view_snapshots() {
    let home = pi_home();
    let project = demo_project();
    let env = env_at(&home);

    insta::assert_json_snapshot!("pi_extensions_user", exported(extensions(&env, None), &home, &project));
    insta::assert_json_snapshot!(
        "pi_extensions_project",
        exported(extensions(&env, Some(&project)), &home, &project)
    );
}

#[test]
fn the_project_trust_notice_accompanies_every_project_view() {
    let home = pi_home();
    let project = demo_project();
    let env = env_at(&home);

    let has_notice =
        |notices: &[harness_business::model::Notice]| notices.iter().any(|notice| notice.message == TRUST_NOTICE);

    assert!(has_notice(&config(&env, Some(&project)).notices));
    assert!(has_notice(&extensions(&env, Some(&project)).notices));
    assert!(!has_notice(&config(&env, None).notices));
}

#[test]
fn credential_files_are_never_listed() {
    let home = pi_home();
    let view = config(&env_at(&home), None);

    let files: Vec<&str> = view
        .sections
        .iter()
        .map(|section| section.source.path.rsplit(['/', '\\']).next().unwrap())
        .collect();

    assert_eq!(
        files,
        [
            "settings.json",
            "models.json",
            "keybindings.json",
            "mcp.json",
            "trust.json"
        ]
    );
}

#[test]
fn model_keys_show_their_secret_kind() {
    let home = pi_home();
    let view = config(&env_at(&home), None);
    let models = view
        .sections
        .iter()
        .find(|section| section.id == "user.models")
        .unwrap();
    let text = serde_json::to_string(&models.entries).unwrap();

    let command = serde_json::to_string(&ConfigValue::Secret(SecretKind::Command)).unwrap();
    let env_ref = serde_json::to_string(&ConfigValue::Secret(SecretKind::EnvRef {
        name: "OPENAI_API_KEY".into(),
    }))
    .unwrap();

    assert!(text.contains(&command));
    assert!(text.contains(&env_ref));
}

#[test]
fn project_mcp_entries_replace_or_override_user_servers() {
    let home = pi_home();
    let project = demo_project();
    let view = extensions(&env_at(&home), Some(&project));
    let server = |name: &str| view.mcp_servers.iter().find(|server| server.name == name).unwrap();

    // `{ "enabled": false }` without an endpoint only switches the user server off.
    assert!(!server("context7").enabled);
    assert_eq!(server("context7").source.layer, ConfigLayer::User);

    assert_eq!(server("project-tools").source.layer, ConfigLayer::Project);
    assert!(!server("filesystem").enabled);
    assert!(server("internal-tools").enabled);
}

#[test]
fn skills_templates_and_packages_come_from_every_pi_location() {
    let home = pi_home();
    let project = demo_project();
    let view = extensions(&env_at(&home), Some(&project));

    let skills: Vec<(&str, ExtensionScope)> = view
        .skills
        .iter()
        .map(|skill| (skill.name.as_str(), skill.scope))
        .collect();
    assert_eq!(
        skills,
        [
            ("pi-project-skill", ExtensionScope::Project),
            ("shared-skill", ExtensionScope::Project),
            ("code-review", ExtensionScope::User),
            ("nested-skill", ExtensionScope::User),
            ("plain-writing", ExtensionScope::User),
            ("local-skill", ExtensionScope::Project),
        ]
    );

    let templates: Vec<(&str, Option<&str>)> = view
        .prompt_templates
        .iter()
        .map(|template| (template.name.as_str(), template.argument_hint.as_deref()))
        .collect();
    assert_eq!(
        templates,
        [
            ("fix", Some("<test name>")),
            ("changelog", None),
            ("review", Some("[focus]"))
        ]
    );

    let packages: Vec<(&str, ExtensionScope, Vec<String>)> = view
        .packages
        .iter()
        .map(|package| (package.kind.as_str(), package.scope, package.resources.clone()))
        .collect();
    assert_eq!(
        packages,
        [
            ("npm", ExtensionScope::User, vec![]),
            ("git", ExtensionScope::User, vec![]),
            ("git", ExtensionScope::User, vec![]),
            ("local", ExtensionScope::Project, vec!["skills".to_owned()]),
        ]
    );
}

#[test]
fn themes_follow_the_light_dark_setting_and_modules_honour_exclusions() {
    let home = pi_home();
    let view = extensions(&env_at(&home), None);

    let active: Vec<&str> = view
        .themes
        .iter()
        .filter(|theme| theme.active)
        .map(|theme| theme.name.as_str())
        .collect();
    assert_eq!(active, ["light", "nightfall"]);

    let modules: Vec<(&str, bool)> = view
        .extension_modules
        .iter()
        .map(|module| (module.name.as_str(), module.enabled))
        .collect();
    assert_eq!(modules, [("legacy", false), ("multi", true), ("permission-gate", true)]);
}
