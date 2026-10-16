//! OMP config and extensions views over the synthetic fixture home.

mod common;

use std::path::Path;

use common::{demo_project, env_at, exported, omp_home};
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harnesses::Omp;
use harness_business::model::{ConfigLayer, ConfigView, ExtensionsView, McpServerInfo, NoticeLevel};
use pretty_assertions::assert_eq;

fn config(env: &Env, project: Option<&Path>) -> ConfigView {
    Omp.config_view(env, project).unwrap()
}

fn extensions(env: &Env, project: Option<&Path>) -> ExtensionsView {
    Omp.extensions_view(env, project).unwrap()
}

fn server<'a>(view: &'a ExtensionsView, name: &str) -> &'a McpServerInfo {
    view.mcp_servers
        .iter()
        .find(|server| server.name == name)
        .unwrap_or_else(|| panic!("server {name} missing"))
}

#[test]
fn config_view_snapshots() {
    let home = omp_home();
    let project = demo_project();
    let env = env_at(&home);

    insta::assert_json_snapshot!("omp_config_user", exported(config(&env, None), &home, &project));
    insta::assert_json_snapshot!(
        "omp_config_project",
        exported(config(&env, Some(&project)), &home, &project)
    );
}

#[test]
fn extensions_view_snapshots() {
    let home = omp_home();
    let project = demo_project();
    let env = env_at(&home);

    insta::assert_json_snapshot!("omp_extensions_user", exported(extensions(&env, None), &home, &project));
    insta::assert_json_snapshot!(
        "omp_extensions_project",
        exported(extensions(&env, Some(&project)), &home, &project)
    );
}

#[test]
fn the_profiles_notice_lists_profile_names_only() {
    let home = omp_home();
    let view = config(&env_at(&home), None);

    let messages: Vec<&str> = view.notices.iter().map(|notice| notice.message.as_str()).collect();
    let profiles = messages
        .iter()
        .find(|message| message.starts_with("Profiles:"))
        .expect("profiles notice");

    assert_eq!(
        *profiles,
        "Profiles: home-lab, work. Start OMP with --profile <name> to use one."
    );
}

#[test]
fn an_active_profile_replaces_the_user_layer() {
    let home = omp_home();
    let env = env_at(&home).with_var("OMP_PROFILE", "work");
    let view = config(&env, None);

    let settings = &view.sections[0];
    assert_eq!(settings.id, "profile.settings");
    assert_eq!(settings.source.layer, ConfigLayer::Profile);
    assert_eq!(settings.source.profile.as_deref(), Some("work"));
    assert!(settings.source.path.ends_with("config.yml"));
    assert!(settings.source.path.contains("profiles"));
    assert_eq!(settings.entries[0].key, "modelRoles");

    let notice = &view.notices[0];
    assert_eq!(notice.message, "Active profile: work. Other profiles: home-lab.");
}

#[test]
fn an_empty_omp_profile_selects_the_default_profile_over_pi_profile() {
    let home = omp_home();
    let env = env_at(&home).with_var("OMP_PROFILE", "").with_var("PI_PROFILE", "work");

    let view = config(&env, None);

    assert_eq!(view.sections[0].id, "user.settings");
}

#[test]
fn an_escaping_catalog_path_is_refused_without_affecting_other_marketplaces() {
    let home = omp_home();
    let view = extensions(&env_at(&home), None);

    let by_name = |name: &str| view.marketplaces.iter().find(|market| market.name == name).unwrap();

    let escape = by_name("escape");
    assert!(escape.entries.is_empty());
    assert_eq!(
        escape.error.as_deref(),
        Some("The catalog path points outside the marketplace cache, so it was not read.")
    );
    assert_eq!(escape.source, "https://••••@git.example.com/team/plugins.git");

    let demo = by_name("demo");
    assert_eq!(demo.error, None);
    let entries: Vec<(&str, bool)> = demo
        .entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry.installed))
        .collect();
    assert_eq!(entries, [("demo-plugin", true), ("commit-helper", false)]);

    let fresh = by_name("fresh");
    assert_eq!(
        fresh.error.as_deref(),
        Some("Catalog not cached yet; refresh the marketplace to load its plugins.")
    );
}

#[test]
fn mcp_servers_follow_omp_enable_rules_and_precedence() {
    let home = omp_home();
    let project = demo_project();
    let view = extensions(&env_at(&home), Some(&project));

    assert!(!server(&view, "postgres").enabled, "disabledServers wins");
    assert!(
        server(&view, "legacy").enabled,
        "enabledServers overrides enabled: false"
    );
    assert!(!server(&view, "paused").enabled, "the string \"false\" is coerced");
    assert!(server(&view, "github").enabled);

    // `.omp/mcp.json` outranks the user file, which outranks the standalone `.mcp.json`.
    assert_eq!(server(&view, "github").command.as_deref(), Some("gh-mcp-project"));
    assert_eq!(server(&view, "github").source.layer, ConfigLayer::Project);
    assert_eq!(server(&view, "linear").source.layer, ConfigLayer::User);
    assert_eq!(server(&view, "vscode-only").command.as_deref(), Some("node"));
}

#[test]
fn plugin_state_combines_the_lock_file_and_project_overrides() {
    let home = omp_home();
    let project = demo_project();
    let env = env_at(&home);

    let user_view = extensions(&env, None);
    let project_view = extensions(&env, Some(&project));
    let plugin = |view: &ExtensionsView, id: &str| view.plugins.iter().find(|plugin| plugin.id == id).cloned().unwrap();
    let features = |view: &ExtensionsView, id: &str| {
        plugin(view, id)
            .features
            .into_iter()
            .map(|feature| (feature.name, feature.enabled))
            .collect::<Vec<_>>()
    };

    assert!(plugin(&user_view, "npm-helper").enabled);
    assert!(
        !plugin(&project_view, "npm-helper").enabled,
        "disabled in plugin-overrides.json"
    );

    assert_eq!(
        features(&user_view, "demo-plugin@demo"),
        [("lint".into(), true), ("format".into(), false)]
    );
    assert_eq!(
        features(&project_view, "demo-plugin@demo"),
        [("lint".into(), false), ("format".into(), true)]
    );
    assert_eq!(
        features(&user_view, "npm-helper"),
        [("fast".into(), true), ("verbose".into(), false)]
    );

    // An installPath outside the plugins directory is listed but never read.
    let escaping = plugin(&user_view, "escaping-plugin@demo");
    assert_eq!(escaping.description, None);
    assert!(escaping.features.is_empty());
}

#[test]
fn skills_come_from_every_omp_location_in_precedence_order() {
    let home = omp_home();
    let project = demo_project();
    let view = extensions(&env_at(&home), Some(&project));

    let skills: Vec<(&str, &str)> = view
        .skills
        .iter()
        .map(|skill| (skill.name.as_str(), skill.origin.as_str()))
        .collect();

    assert_eq!(
        skills,
        [
            ("project-skill", ".omp/skills"),
            ("demo-skill", "~/.omp/agent/skills"),
            ("plugin-skill", "plugin demo-plugin"),
            ("auto-notes", "~/.omp/agent/managed-skills"),
        ]
    );
}

#[test]
fn parse_errors_are_reported_by_location_only() {
    let home = omp_home();
    let project = demo_project();
    let view = config(&env_at(&home), Some(&project));

    let broken = view
        .sections
        .iter()
        .find(|section| section.id == "project.settingsJson")
        .unwrap();

    assert!(broken.entries.is_empty());
    assert_eq!(broken.error.as_deref(), Some("Invalid JSON at line 4, column 1"));
    assert_eq!(view.notices.last().unwrap().level, NoticeLevel::Info);
}

#[cfg(unix)]
#[test]
fn a_symlinked_skills_dir_with_loops_lists_each_skill_once() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let shared = temp.path().join("shared-skills");
    let skill = shared.join("notes");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join("SKILL.md"), "---\ndescription: Notes\n---\n").unwrap();
    std::fs::create_dir_all(home.join(".omp/agent")).unwrap();
    symlink(&shared, home.join(".omp/agent/skills")).unwrap();
    for index in 0..50 {
        symlink(".", shared.join(format!("loop-{index}"))).unwrap();
    }

    let view = extensions(&env_at(&home), None);

    let names: Vec<&str> = view.skills.iter().map(|skill| skill.name.as_str()).collect();
    assert_eq!(names, ["notes"]);
}
