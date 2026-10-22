//! OMP config and extensions views over the synthetic fixture home.

use crate::common;

use std::path::Path;

use common::{big_dirs, demo_project, env_at, exported, omp_home, within, write_file};
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

    let messages: Vec<&str> = view.notices.iter().map(|notice| notice.message.as_str()).collect();
    assert_eq!(messages[..2], ["Active profile: work.", "Other profiles: home-lab."]);
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

#[cfg(unix)]
#[test]
fn settings_listed_module_paths_are_deduplicated_then_capped_per_file() {
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let modules = home.join("modules");
    let agent = home.join(".omp/agent");
    let project = temp.path().join("project");
    std::fs::create_dir_all(&modules).unwrap();
    std::fs::create_dir_all(&agent).unwrap();
    std::fs::create_dir_all(project.join(".git")).unwrap();
    std::os::unix::fs::symlink(&modules, home.join("linked")).unwrap();

    // 100 distinct files, each listed four ways: absolute, with `~` (the same path once expanded),
    // through a linked directory and relative to the project.
    let mut listed = Vec::new();
    for index in 0..100 {
        let name = format!("m{index:03}.ts");
        let file = modules.join(&name);
        std::fs::write(&file, "").unwrap();
        listed.push(file.to_string_lossy().into_owned());
        listed.push(format!("~/modules/{name}"));
        listed.push(format!("~/linked/{name}"));
        listed.push(format!("../home/modules/{name}"));
    }
    let yaml: String = listed.iter().map(|path| format!("  - {path}\n")).collect();
    std::fs::write(agent.join("config.yml"), format!("extensions:\n{yaml}")).unwrap();
    let legacy = serde_json::json!({ "extensions": listed.iter().rev().collect::<Vec<_>>() });
    std::fs::write(agent.join("settings.json"), legacy.to_string()).unwrap();

    let view = extensions(&env_at(&home), Some(&project));

    let names: Vec<&str> = view
        .extension_modules
        .iter()
        .map(|module| module.name.as_str())
        .collect();
    // Each file has three distinct spellings, so a file's first 64 entries name 22 files: the
    // legacy file m099 down to m078, the YAML file m000 to m021. Aliases count once.
    let expected: Vec<String> = (78..100)
        .rev()
        .chain(0..22)
        .map(|index| format!("m{index:03}"))
        .collect();
    assert_eq!(names, expected);
    assert!(has_truncation_notice(&view, USER_TRUNCATED));
}

const PROJECT_TRUNCATED: &str = "Some extensions were not listed because this project lists more than the app reads. \
                                 Check its settings before trusting it.";
const USER_TRUNCATED: &str = "Some extensions were not listed because your setup lists more than the app reads.";

fn has_truncation_notice(view: &ExtensionsView, message: &str) -> bool {
    view.notices.iter().any(|notice| notice.message == message)
}

#[test]
fn a_project_listing_too_much_cannot_hide_the_users_modules() {
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let agent = home.join(".omp/agent");
    let project = temp.path().join("project");
    std::fs::create_dir_all(project.join(".git")).unwrap();

    write_file(&project.join(".omp/extensions/evil.ts"), "");
    write_file(&agent.join("extensions/user-mod.ts"), "");
    write_file(&home.join("mods/listed-user.ts"), "");
    write_file(&agent.join("config.yml"), "extensions:\n  - ~/mods\n");

    // 21 directories of 2000 files are more than one layer's 40 000 scan steps.
    let listed = big_dirs(&temp.path().join("big"), 21, 2000, "");
    let legacy = serde_json::json!({ "extensions": listed });
    write_file(&project.join(".omp/settings.json"), &legacy.to_string());

    let view = within(5.0, || extensions(&env_at(&home), Some(&project)));

    let names: Vec<&str> = view
        .extension_modules
        .iter()
        .map(|module| module.name.as_str())
        .collect();
    assert_eq!(names, ["evil", "user-mod", "listed-user"]);
    assert!(has_truncation_notice(&view, PROJECT_TRUNCATED));
    assert!(!has_truncation_notice(&view, USER_TRUNCATED));
}

/// A `SKILL.md` without a `name` whose frontmatter holds `keys` extra keys of 12 bytes each.
fn padded_skill(dir: &Path, keys: usize) {
    let padding: String = (0..keys).map(|key| format!("key{key:05}: v\n")).collect();
    write_file(
        &dir.join("SKILL.md"),
        &format!("---\ndescription: A skill\n{padding}---\n"),
    );
}

#[cfg(unix)]
#[test]
fn links_to_one_skill_read_it_once_and_huge_frontmatter_is_skipped() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let skills = home.join(".omp/agent/skills");
    let shared = temp.path().join("shared");
    let huge = temp.path().join("huge");
    std::fs::create_dir_all(&skills).unwrap();

    // Just under the 64 KiB frontmatter cap, and well over it (but under the 256 KiB file cap).
    padded_skill(&shared, 4800);
    padded_skill(&huge, 19_000);
    for index in 0..1000 {
        symlink(&shared, skills.join(format!("a{index:04}"))).unwrap();
        symlink(&huge, skills.join(format!("b{index:04}"))).unwrap();
    }

    let view = within(2.0, || extensions(&env_at(&home), None));

    // Named after the first link: the others resolve to the same directory.
    let names: Vec<&str> = view.skills.iter().map(|skill| skill.name.as_str()).collect();
    assert_eq!(names, ["a0000"]);
}
