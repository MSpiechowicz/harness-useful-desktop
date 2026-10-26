//! PI packages installed from git and npm: install directories, resources and the skills they
//! provide, over temporary homes.

use crate::common;

use std::path::{Path, PathBuf};

use common::{env_at, write_file};
use harness_business::harness::Harness;
use harness_business::harnesses::Pi;
use harness_business::model::{ExtensionScope, ExtensionsView, PackageInfo, SkillInfo};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const SKILLS_REPO: &str = "git:github.com/MSpiechowicz/harness-useful-skills";

/// A temporary home (and project) for one test.
struct Fixture {
    _temp: TempDir,
    home: PathBuf,
    project: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        let home = root.join("home");
        let project = root.join("work/demo");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(project.join(".git")).unwrap();

        Self {
            _temp: temp,
            home,
            project,
        }
    }

    fn agent_dir(&self) -> PathBuf {
        self.home.join(".pi/agent")
    }

    fn user_settings(&self, settings: serde_json::Value) {
        write_file(&self.agent_dir().join("settings.json"), &settings.to_string());
    }

    fn project_settings(&self, settings: serde_json::Value) {
        write_file(&self.project.join(".pi/settings.json"), &settings.to_string());
    }

    /// The checkout PI clones the harness-useful-skills repository into: no `pi` manifest (only an
    /// `omp` one), skills in the conventional directory.
    fn install_skills_repo(&self) -> PathBuf {
        let dir = self
            .agent_dir()
            .join("git/github.com/MSpiechowicz/harness-useful-skills");
        write_file(
            &dir.join("package.json"),
            r#"{ "name": "harness-useful-skills", "omp": { "extensions": ["./extension.js"] } }"#,
        );
        write_file(&dir.join("extension.js"), "export default function () {}");
        write_skill(&dir.join("skills/us-implement"), "us-implement");
        write_skill(&dir.join("skills/us-grill-me"), "us-grill-me");

        dir
    }

    /// The user view with harness-useful-skills installed and `packages` as the user's list.
    fn skills_repo_view(&self, packages: serde_json::Value) -> ExtensionsView {
        self.user_settings(serde_json::json!({ "packages": packages }));
        self.install_skills_repo();

        self.view(false)
    }

    fn view(&self, project: bool) -> ExtensionsView {
        let project = project.then_some(self.project.as_path());

        Pi.extensions_view(&env_at(&self.home), project).unwrap()
    }
}

fn write_skill(dir: &Path, name: &str) {
    write_file(
        &dir.join("SKILL.md"),
        &format!("---\nname: {name}\ndescription: The {name} skill.\n---\n"),
    );
}

fn package_names(view: &ExtensionsView) -> Vec<&str> {
    view.packages.iter().map(|package| package.name.as_str()).collect()
}

fn package<'a>(view: &'a ExtensionsView, name: &str) -> &'a PackageInfo {
    view.packages.iter().find(|package| package.name == name).unwrap()
}

/// `(name, origin, scope, package)` of every skill.
fn skills(view: &ExtensionsView) -> Vec<(String, String, ExtensionScope, Option<String>)> {
    view.skills
        .iter()
        .map(|skill: &SkillInfo| {
            (
                skill.name.clone(),
                skill.origin.clone(),
                skill.scope,
                skill.package.clone(),
            )
        })
        .collect()
}

fn from_package(name: &str, package: &str, scope: ExtensionScope) -> (String, String, ExtensionScope, Option<String>) {
    (
        name.to_owned(),
        format!("package {package}"),
        scope,
        Some(package.to_owned()),
    )
}

#[test]
fn a_user_git_package_lists_its_skills_from_the_clone() {
    let fixture = Fixture::new();
    fixture.user_settings(serde_json::json!({ "packages": [SKILLS_REPO] }));
    let dir = fixture.install_skills_repo();

    let view = fixture.view(false);

    let installed = package(&view, "harness-useful-skills");
    assert_eq!(installed.kind, "git");
    assert_eq!(installed.resources, ["skills"]);

    assert_eq!(
        skills(&view),
        [
            from_package("us-grill-me", "harness-useful-skills", ExtensionScope::User),
            from_package("us-implement", "harness-useful-skills", ExtensionScope::User),
        ]
    );
    let first = dir.join("skills/us-grill-me/SKILL.md");
    // Compared as paths: on Windows the expected path mixes `/` and `\`.
    assert_eq!(Path::new(&view.skills[0].path), first);

    // The `omp` manifest is not PI's: its extension is not a PI module.
    assert!(view.extension_modules.is_empty());
}

#[test]
fn pins_and_url_forms_share_the_same_clone() {
    let fixture = Fixture::new();
    fixture.install_skills_repo();
    fixture.user_settings(serde_json::json!({
        "packages": ["https://github.com/MSpiechowicz/harness-useful-skills.git@v0.3.3"]
    }));

    let view = fixture.view(false);

    assert_eq!(package(&view, "harness-useful-skills").resources, ["skills"]);
    assert_eq!(view.skills.len(), 2);
}

#[test]
fn a_package_that_is_not_installed_provides_nothing() {
    let fixture = Fixture::new();
    fixture.user_settings(serde_json::json!({ "packages": [SKILLS_REPO, "npm:@demo/kit@1.0.0"] }));

    let view = fixture.view(false);

    assert!(package(&view, "harness-useful-skills").resources.is_empty());
    assert!(package(&view, "@demo/kit").resources.is_empty());
    assert!(view.skills.is_empty());
}

#[test]
fn project_git_and_npm_packages_install_under_the_project_pi_dir() {
    let fixture = Fixture::new();
    fixture.project_settings(serde_json::json!({
        "packages": ["git:git.example.com/team/review-tools@main", "npm:@team/pi-kit@^2"]
    }));
    write_skill(
        &fixture
            .project
            .join(".pi/git/git.example.com/team/review-tools/skills/review"),
        "review",
    );
    let kit = fixture.project.join(".pi/npm/node_modules/@team/pi-kit");
    write_file(
        &kit.join("package.json"),
        r#"{ "name": "@team/pi-kit", "pi": { "skills": ["./resources/skills"], "prompts": ["./prompts/*.md"] } }"#,
    );
    write_skill(&kit.join("resources/skills/kit-skill"), "kit-skill");
    write_skill(&kit.join("skills/undeclared"), "undeclared");
    write_file(&kit.join("prompts/triage.md"), "---\ndescription: Triage.\n---\n");

    let view = fixture.view(true);

    assert_eq!(package(&view, "review-tools").resources, ["skills"]);
    assert_eq!(package(&view, "@team/pi-kit").resources, ["skills", "prompts"]);
    assert_eq!(
        skills(&view),
        [
            from_package("review", "review-tools", ExtensionScope::Project),
            from_package("kit-skill", "@team/pi-kit", ExtensionScope::Project),
        ]
    );

    let templates: Vec<(&str, ExtensionScope)> = view
        .prompt_templates
        .iter()
        .map(|template| (template.name.as_str(), template.scope))
        .collect();
    assert_eq!(templates, [("triage", ExtensionScope::Project)]);
}

#[test]
fn an_empty_skills_filter_hides_the_package_skills() {
    let fixture = Fixture::new();
    let view = fixture.skills_repo_view(serde_json::json!([{ "source": SKILLS_REPO, "skills": [] }]));

    assert!(package(&view, "harness-useful-skills").resources.is_empty());
    assert!(view.skills.is_empty());
}

#[test]
fn the_harness_own_skills_win_over_package_skills() {
    let fixture = Fixture::new();
    fixture.user_settings(serde_json::json!({ "packages": [SKILLS_REPO] }));
    fixture.install_skills_repo();
    write_skill(&fixture.agent_dir().join("skills/us-implement"), "us-implement");

    let view = fixture.view(false);

    assert_eq!(
        skills(&view),
        [
            (
                "us-implement".to_owned(),
                "~/.pi/agent/skills".to_owned(),
                ExtensionScope::User,
                None
            ),
            from_package("us-grill-me", "harness-useful-skills", ExtensionScope::User),
        ]
    );
}

#[cfg(unix)]
#[test]
fn nothing_outside_the_package_directory_is_read() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let outside = fixture.home.join("outside");
    write_skill(&outside.join("secret-skill"), "secret-skill");
    write_skill(&outside.join("bundle/linked-skill"), "linked-skill");

    // Escapes through a linked skill, a linked conventional directory and a manifest entry.
    let linked = fixture.agent_dir().join("git/github.com/acme/linked");
    write_skill(&linked.join("skills/kept"), "kept");
    symlink(outside.join("secret-skill"), linked.join("skills/secret-skill")).unwrap();

    let whole_dir = fixture.agent_dir().join("git/github.com/acme/whole-dir");
    std::fs::create_dir_all(&whole_dir).unwrap();
    symlink(outside.join("bundle"), whole_dir.join("skills")).unwrap();

    let manifest = fixture.agent_dir().join("git/github.com/acme/manifest");
    write_file(
        &manifest.join("package.json"),
        r#"{ "pi": { "skills": ["../../../../../outside", "/etc"] } }"#,
    );

    // The clone itself links out of the install root.
    std::fs::create_dir_all(fixture.agent_dir().join("git/github.com/acme")).unwrap();
    symlink(&outside, fixture.agent_dir().join("git/github.com/acme/clone")).unwrap();

    fixture.user_settings(serde_json::json!({
        "packages": [
            "git:github.com/acme/linked",
            "git:github.com/acme/whole-dir",
            "git:github.com/acme/manifest",
            "git:github.com/acme/clone",
            "git:github.com/acme/../../../outside",
        ]
    }));

    let view = fixture.view(false);

    assert_eq!(skills(&view), [from_package("kept", "linked", ExtensionScope::User)]);

    let resources: Vec<(&str, usize)> = view
        .packages
        .iter()
        .map(|package| (package.name.as_str(), package.resources.len()))
        .collect();
    assert_eq!(
        resources,
        [
            ("linked", 1),
            ("whole-dir", 0),
            ("manifest", 0),
            ("clone", 0),
            ("outside", 0)
        ]
    );
}

#[test]
fn package_names_never_carry_query_strings_or_tokens() {
    let fixture = Fixture::new();
    // Built at run time: a token-shaped literal in the source trips secret scanning on push.
    let gitlab = format!(
        "https://gitlab.example.com/acme/tools.git?private_token={}-AbCdEf0123456789xyzQ",
        "glpat"
    );
    let sources = [
        gitlab.as_str(),
        "git:host.example/acme/tools?access_token=s3cr3tV4lu3xQ9z",
        "https://host.example/acme/tools/?token=abc",
        "git:ci:hunter2@host.example/acme/tools",
    ];
    fixture.user_settings(serde_json::json!({ "packages": sources }));
    write_skill(
        &fixture
            .agent_dir()
            .join("git/gitlab.example.com/acme/tools/skills/lint"),
        "lint",
    );

    let view = fixture.view(false);

    let names = package_names(&view);
    assert_eq!(names, ["tools", "tools", "tools", "tools"]);

    let labels = view
        .skills
        .iter()
        .flat_map(|skill| [skill.origin.clone(), skill.package.clone().unwrap_or_default()]);
    for label in names.iter().map(|name| (*name).to_owned()).chain(labels) {
        for leak in ["?", "token", "glpat-", "hunter2", "s3cr3t"] {
            assert!(!label.contains(leak), "{label:?} carries {leak:?}");
        }
    }

    // The query is not part of the clone path, so the URL resolves to the installed clone.
    assert_eq!(skills(&view), [from_package("lint", "tools", ExtensionScope::User)]);
    assert!(view.packages[0].installed);
}

#[test]
fn a_name_that_looks_like_a_secret_is_replaced_by_the_kind() {
    let fixture = Fixture::new();
    fixture.user_settings(serde_json::json!({
        "packages": [
            "https://host.example/acme/ghp_AbCdEfGhIjKlMnOpQrStUvWxYz0123456789",
            "../tools/Zx8Qp2Lm9Rt4Vb7Nc1Kd6Hf3Jg5Ws0Ye",
            "../tools/name with spaces",
        ]
    }));

    assert_eq!(package_names(&fixture.view(false)), ["git", "local", "local"]);
}

#[test]
fn repeated_package_and_manifest_entries_stay_bounded() {
    let fixture = Fixture::new();
    let package = fixture.project.join(".pi/p");
    // Repeats only, and small enough (about 170 KiB) to be read.
    let manifest = serde_json::json!({ "pi": { "skills": vec!["s"; 15_000], "prompts": vec!["./prompts/"; 8_000] } });
    write_file(&package.join("package.json"), &manifest.to_string());
    write_skill(&package.join("s"), "only");
    write_file(&package.join("prompts/note.md"), "---\ndescription: A note.\n---\n");

    let mut entries = vec![serde_json::json!("./p"); 200_000];
    entries.push(serde_json::json!("./p/"));
    entries.push(serde_json::json!({ "source": "./p" }));
    fixture.project_settings(serde_json::json!({ "packages": entries, "skills": vec!["./p/s"; 50_000] }));

    let started = std::time::Instant::now();
    let view = fixture.view(true);
    let elapsed = started.elapsed();

    assert_eq!(view.packages.len(), 1);
    assert_eq!(view.packages[0].resources, ["skills", "prompts"]);
    assert_eq!(view.skills.len(), 1);
    assert_eq!(view.prompt_templates.len(), 1);
    assert!(elapsed < std::time::Duration::from_secs(10), "took {elapsed:?}");

    // Repeats are not more than the app reads.
    assert!(
        view.notices
            .iter()
            .all(|notice| !notice.message.starts_with("Some extensions"))
    );
}

#[test]
fn a_manifest_over_256_kib_is_not_read() {
    let fixture = Fixture::new();
    let package = fixture.project.join(".pi/big");
    let padding = "x".repeat(256 * 1024);
    let manifest = serde_json::json!({ "pi": { "skills": ["./custom"] }, "pad": padding });
    write_file(&package.join("package.json"), &manifest.to_string());
    write_skill(&package.join("custom/declared"), "declared");
    write_skill(&package.join("skills/conventional"), "conventional");
    fixture.project_settings(serde_json::json!({ "packages": ["./big"] }));

    let view = fixture.view(true);

    // Without its manifest the package falls back to the conventional directories.
    let names: Vec<&str> = view.skills.iter().map(|skill| skill.name.as_str()).collect();
    assert_eq!(names, ["conventional"]);
}

#[test]
fn a_padded_source_is_the_same_package_as_the_plain_one() {
    let fixture = Fixture::new();
    let padded = format!("  {SKILLS_REPO} ");
    let view = fixture.skills_repo_view(serde_json::json!([padded, SKILLS_REPO]));

    assert_eq!(view.packages.len(), 1);
    let installed = package(&view, "harness-useful-skills");
    assert_eq!(installed.kind, "git");
    assert!(installed.installed);
}

#[test]
fn project_packages_cannot_crowd_out_the_users_package_modules() {
    let fixture = Fixture::new();

    // Five project packages of 64 modules each are more than the 256 paths one layer loads.
    let mut sources = Vec::new();
    for package in 0..5 {
        let dir = fixture.project.join(format!(".pi/p{package}"));
        let modules: Vec<String> = (0..64).map(|module| format!("./m{module:02}.ts")).collect();
        for module in &modules {
            write_file(&dir.join(module), "");
        }
        write_file(
            &dir.join("package.json"),
            &serde_json::json!({ "pi": { "extensions": modules } }).to_string(),
        );
        sources.push(format!("./p{package}"));
    }
    fixture.project_settings(serde_json::json!({ "packages": sources }));

    let mine = fixture.agent_dir().join("mine");
    write_file(&mine.join("mine.ts"), "");
    write_file(
        &mine.join("package.json"),
        r#"{ "pi": { "extensions": ["./mine.ts"] } }"#,
    );
    fixture.user_settings(serde_json::json!({ "packages": ["./mine"] }));

    let view = fixture.view(true);

    let project_modules = view
        .extension_modules
        .iter()
        .filter(|module| module.scope == ExtensionScope::Project)
        .count();
    assert_eq!(project_modules, 256);
    assert!(
        view.extension_modules
            .iter()
            .any(|module| module.name == "mine" && module.scope == ExtensionScope::User)
    );
    assert!(view.notices.iter().any(|notice| {
        notice
            .message
            .starts_with("Some extensions were not listed because this project")
    }));
}
