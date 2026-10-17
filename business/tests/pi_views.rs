//! PI config and extensions views over the synthetic fixture home.

mod common;

use std::path::Path;

use common::{big_dirs, demo_project, env_at, exported, pi_home, within, write_file};
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
            ("release-notes", ExtensionScope::User),
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
            ("review", Some("[focus]")),
            ("standup", None)
        ]
    );

    let packages: Vec<(&str, &str, ExtensionScope, Vec<String>)> = view
        .packages
        .iter()
        .map(|package| {
            (
                package.kind.as_str(),
                package.name.as_str(),
                package.scope,
                package.resources.clone(),
            )
        })
        .collect();
    assert_eq!(
        packages,
        [
            (
                "npm",
                "@demo/pi-review-kit",
                ExtensionScope::User,
                vec!["themes".to_owned()]
            ),
            ("git", "pi-prompts", ExtensionScope::User, vec!["prompts".to_owned()]),
            ("git", "pi-private", ExtensionScope::User, vec!["skills".to_owned()]),
            ("local", "pi-local", ExtensionScope::Project, vec!["skills".to_owned()]),
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

#[cfg(unix)]
#[test]
fn skill_symlink_loops_are_skipped_and_linked_skills_kept() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let skills = home.join(".agents/skills");
    let shared = temp.path().join("shared/linked");
    write_skill(&skills.join("group/grouped"));
    write_skill(&shared);
    symlink(&shared, skills.join("linked")).unwrap();
    for index in 0..50 {
        symlink(".", skills.join(format!("loop-{index}"))).unwrap();
        symlink("..", skills.join("group").join(format!("up-{index}"))).unwrap();
    }

    let view = within(2.0, || extensions(&env_at(&home), None));

    let mut names: Vec<&str> = view.skills.iter().map(|skill| skill.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["grouped", "linked"]);
}

#[cfg(unix)]
#[test]
fn listed_aliases_of_a_linked_hub_are_scanned_once() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let agent = home.join(".pi/agent");
    let target = temp.path().join("target");
    let hub = temp.path().join("hub");
    std::fs::create_dir_all(&agent).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    std::fs::create_dir_all(&hub).unwrap();

    let padding = "x".repeat(100 * 1024);
    let manifest = format!(r#"{{"pi":{{"extensions":["./main.ts"]}},"pad":"{padding}"}}"#);
    std::fs::write(target.join("package.json"), manifest).unwrap();
    std::fs::write(target.join("main.ts"), "").unwrap();
    std::fs::write(hub.join("note.md"), "---\ndescription: A note.\n---\n").unwrap();
    std::fs::write(hub.join("hub-theme.json"), "{}").unwrap();
    for index in 0..1998 {
        symlink(&target, hub.join(format!("e{index:04}"))).unwrap();
    }

    // `hub`, `hub/../hub`, `hub/../hub/../hub`, ... are 64 distinct entries for one directory.
    let aliases: Vec<String> = (0..64)
        .map(|depth| format!("{}{}", hub.display(), "/../hub".repeat(depth)))
        .collect();
    let settings = serde_json::json!({ "extensions": aliases, "prompts": aliases, "themes": aliases });
    std::fs::write(agent.join("settings.json"), settings.to_string()).unwrap();

    let view = within(2.0, || extensions(&env_at(&home), None));

    let modules: Vec<&str> = view
        .extension_modules
        .iter()
        .map(|module| module.name.as_str())
        .collect();
    assert_eq!(modules, ["main"]);

    // Every alias lists the hub's note and theme again, so one of each needs the aliases deduped.
    let prompts: Vec<&str> = view
        .prompt_templates
        .iter()
        .map(|prompt| prompt.name.as_str())
        .collect();
    assert_eq!(prompts, ["note"]);
    let custom_themes: Vec<&str> = view
        .themes
        .iter()
        .filter(|theme| theme.scope != ExtensionScope::Builtin)
        .map(|theme| theme.name.as_str())
        .collect();
    assert_eq!(custom_themes, ["hub-theme"]);
}

/// `count` directories under `root`, each holding `per_dir` empty files named `p0000<suffix>`, ...
#[test]
fn a_project_listing_too_much_cannot_hide_modules_or_user_resources() {
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let agent = home.join(".pi/agent");
    let project = temp.path().join("project");
    std::fs::create_dir_all(project.join(".git")).unwrap();

    write_file(&project.join(".pi/extensions/evil.ts"), "");
    write_file(&agent.join("extensions/user-mod.ts"), "");
    write_file(&home.join("mods/listed-user.ts"), "");
    write_file(&agent.join("prompts/user-prompt.md"), "");
    let user_settings = serde_json::json!({ "extensions": ["~/mods"] });
    write_file(&agent.join("settings.json"), &user_settings.to_string());

    // 21 x 2000 entries are more than one layer's 40 000 scan steps.
    let listed = big_dirs(&temp.path().join("big"), 21, 2000, ".md");
    let project_settings = serde_json::json!({ "extensions": listed, "prompts": listed });
    write_file(&project.join(".pi/settings.json"), &project_settings.to_string());

    let view = within(5.0, || extensions(&env_at(&home), Some(&project)));

    let modules: Vec<(&str, ExtensionScope)> = view
        .extension_modules
        .iter()
        .map(|module| (module.name.as_str(), module.scope))
        .collect();
    assert_eq!(
        modules,
        [
            ("evil", ExtensionScope::Project),
            ("user-mod", ExtensionScope::User),
            ("listed-user", ExtensionScope::User),
        ]
    );
    assert!(
        view.prompt_templates
            .iter()
            .any(|prompt| prompt.name == "user-prompt" && prompt.scope == ExtensionScope::User)
    );

    let truncated: Vec<&str> = view
        .notices
        .iter()
        .map(|notice| notice.message.as_str())
        .filter(|message| message.starts_with("Some extensions"))
        .collect();
    assert_eq!(
        truncated,
        [
            "Some extensions were not listed because this project lists more than the app reads. Check its \
          settings before trusting it."
        ]
    );
}

fn write_skill(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), "---\ndescription: A skill\n---\n").unwrap();
}
