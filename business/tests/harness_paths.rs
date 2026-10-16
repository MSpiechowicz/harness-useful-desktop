//! Directory resolution (OMP `dirs.ts` rules, PI agent dir, OpenCode XDG directories) and the
//! shared parsing helpers.

use std::fs;
use std::path::PathBuf;

use harness_business::env::{Env, OsKind};
use harness_business::harness::Harness;
use harness_business::harnesses::omp::{active_profile, omp_paths};
use harness_business::harnesses::opencode::{custom_config_dir, data_dir, home_config_dir};
use harness_business::harnesses::pi::pi_paths;
use harness_business::harnesses::{Omp, OpenCode, Pi};
use harness_business::model::ConfigFormat;
use harness_business::pi_family::{parse_frontmatter, parse_text, project_ancestors, text_field};
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

/// An absolute home on every OS (it does not need to exist).
fn home() -> PathBuf {
    std::env::temp_dir().join("harness-paths-demo-home")
}

fn custom_agent() -> PathBuf {
    home().join("srv/agent")
}

#[test]
fn omp_defaults_to_the_dot_omp_agent_dir() {
    let project = home().join("Projects/demo");
    let paths = omp_paths(&Env::for_tests(home()), Some(&project));

    assert_eq!(paths.config_root, home().join(".omp"));
    assert_eq!(paths.agent_dir, home().join(".omp/agent"));
    assert_eq!(paths.data_root, home().join(".omp"));
    assert_eq!(paths.profile, None);
    assert_eq!(paths.project_dir, Some(project.join(".omp")));
}

#[test]
fn omp_honours_config_dir_agent_dir_and_profiles() {
    let renamed = omp_paths(&Env::for_tests(home()).with_var("PI_CONFIG_DIR", ".omp-dev"), None);
    assert_eq!(renamed.agent_dir, home().join(".omp-dev/agent"));

    let agent_var = custom_agent().to_string_lossy().into_owned();
    let custom = omp_paths(
        &Env::for_tests(home()).with_var("PI_CODING_AGENT_DIR", &agent_var),
        None,
    );
    assert_eq!(custom.agent_dir, custom_agent());
    assert_eq!(custom.config_root, home().join(".omp"));

    let profiled = Env::for_tests(home())
        .with_var("OMP_PROFILE", "work")
        .with_var("PI_CODING_AGENT_DIR", &agent_var);
    let paths = omp_paths(&profiled, None);
    assert_eq!(paths.profile.as_deref(), Some("work"));
    assert_eq!(paths.agent_dir, home().join(".omp/profiles/work/agent"));
    assert_eq!(paths.data_root, home().join(".omp/profiles/work"));
}

#[test]
fn invalid_profile_names_fall_back_to_the_default_profile() {
    for name in ["Work", "../escape", "default", "con", "a.", " "] {
        let env = Env::for_tests(home()).with_var("OMP_PROFILE", name);
        assert_eq!(active_profile(&env), None, "{name:?}");
    }

    let legacy = Env::for_tests(home()).with_var("PI_PROFILE", "lab");
    assert_eq!(active_profile(&legacy).as_deref(), Some("lab"));
}

#[test]
fn an_existing_xdg_data_dir_holds_omp_plugins_on_unix_like_systems() {
    let temp = TempDir::new().unwrap();
    let xdg = temp.path().join("data");
    fs::create_dir_all(xdg.join("omp")).unwrap();
    let env = Env::for_tests(temp.path()).with_var("XDG_DATA_HOME", xdg.to_string_lossy());

    let linux = omp_paths(&env.clone().with_os(OsKind::Linux), None);
    assert_eq!(linux.data_root, xdg.join("omp"));
    assert_eq!(linux.agent_dir, temp.path().join(".omp/agent"));

    let windows = omp_paths(&env.clone().with_os(OsKind::Windows), None);
    assert_eq!(windows.data_root, temp.path().join(".omp"));

    let overridden = env
        .with_os(OsKind::Linux)
        .with_var("PI_CODING_AGENT_DIR", custom_agent().to_string_lossy());
    assert_eq!(omp_paths(&overridden, None).data_root, temp.path().join(".omp"));
}

#[test]
fn pi_uses_its_agent_dir_variable() {
    let default = pi_paths(&Env::for_tests(home()), None);
    assert_eq!(default.config_root, home().join(".pi"));
    assert_eq!(default.agent_dir, home().join(".pi/agent"));

    let custom = pi_paths(
        &Env::for_tests(home()).with_var("PI_CODING_AGENT_DIR", "~/pi-agent"),
        None,
    );
    assert_eq!(custom.agent_dir, home().join("pi-agent"));
    assert_eq!(custom.config_root, home().join("pi-agent"));
}

#[test]
fn opencode_defaults_to_the_xdg_fallbacks_below_home() {
    let project = home().join("Projects/demo");
    let paths = OpenCode.paths(&Env::for_tests(home()), Some(&project));

    assert_eq!(paths.config_root, home().join(".config/opencode"));
    assert_eq!(paths.agent_dir, paths.config_root);
    assert_eq!(paths.data_root, home().join(".local/share/opencode"));
    assert_eq!(paths.profile, None);
    assert_eq!(paths.project_dir, Some(project.join(".opencode")));
    assert_eq!(home_config_dir(&Env::for_tests(home())), home().join(".opencode"));
}

#[test]
fn opencode_honours_absolute_xdg_directories_only() {
    let config = home().join("xdg/config");
    let data = home().join("xdg/data");
    let env = Env::for_tests(home())
        .with_var("XDG_CONFIG_HOME", config.to_string_lossy())
        .with_var("XDG_DATA_HOME", data.to_string_lossy());

    let paths = OpenCode.paths(&env, None);
    assert_eq!(paths.config_root, config.join("opencode"));
    assert_eq!(paths.data_root, data.join("opencode"));
    assert_eq!(OpenCode.sessions_dir(&env, &home()), data.join("opencode"));

    let relative = Env::for_tests(home())
        .with_var("XDG_CONFIG_HOME", "xdg/config")
        .with_var("XDG_DATA_HOME", "xdg/data");
    assert_eq!(
        OpenCode.paths(&relative, None).config_root,
        home().join(".config/opencode")
    );
    assert_eq!(data_dir(&relative), home().join(".local/share/opencode"));
}

#[test]
fn the_opencode_config_dir_variable_adds_a_layer_without_moving_the_root() {
    let team = home().join("team-opencode");
    let env = Env::for_tests(home()).with_var("OPENCODE_CONFIG_DIR", team.to_string_lossy());

    assert_eq!(OpenCode.paths(&env, None).config_root, home().join(".config/opencode"));
    assert_eq!(custom_config_dir(&env), Some(team));

    let relative = Env::for_tests(home()).with_var("OPENCODE_CONFIG_DIR", "team-opencode");
    assert_eq!(custom_config_dir(&relative), None);
}

#[test]
fn adapters_report_their_paths_through_the_trait() {
    let env = Env::for_tests(home());

    assert_eq!(Omp.paths(&env, None), omp_paths(&env, None));
    assert_eq!(Pi.paths(&env, None), pi_paths(&env, None));
}

#[test]
fn the_ancestor_walk_stops_at_the_repository_root_and_never_includes_home() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("work/repo");
    let nested = repo.join("packages/app");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(repo.join(".git")).unwrap();

    let env = Env::for_tests(temp.path().join("work"));
    assert_eq!(
        project_ancestors(&env, &nested),
        [nested.clone(), repo.join("packages"), repo.clone()]
    );

    let home_env = Env::for_tests(&repo);
    assert_eq!(
        project_ancestors(&home_env, &nested),
        [nested.clone(), repo.join("packages")]
    );
}

#[test]
fn frontmatter_is_parsed_from_the_leading_block_only() {
    let parsed = parse_frontmatter("\u{feff}---\r\nname: x\r\ndescription: A skill\r\n---\r\nbody: ignored\n").unwrap();
    assert_eq!(parsed, json!({ "name": "x", "description": "A skill" }));
    assert_eq!(text_field(&parsed, "description").as_deref(), Some("A skill"));

    assert_eq!(parse_frontmatter("no frontmatter\n---\nname: x\n---\n"), None);
    assert_eq!(parse_frontmatter("---\nname: x\n"), None, "unclosed block");
    assert_eq!(parse_frontmatter("---\n- a list\n---\n"), None, "not a mapping");

    let block = |keys: usize| -> String { (0..keys).map(|key| format!("key{key:05}: v\n")).collect() };
    assert!(
        parse_frontmatter(&format!("---\n{}---\n", block(5400))).is_some(),
        "64 800 bytes"
    );
    assert_eq!(
        parse_frontmatter(&format!("---\n{}---\n", block(5500))),
        None,
        "66 000 bytes"
    );
}

#[test]
fn parse_errors_name_a_location_but_no_content() {
    let yaml = parse_text("a: 1\nsecret: [unclosed\n", ConfigFormat::Yaml).unwrap_err();
    assert!(yaml.starts_with("Invalid YAML at line"), "{yaml}");
    assert!(!yaml.contains("secret") && !yaml.contains("unclosed"));

    let json = parse_text("{\"token\": \"abc\",}", ConfigFormat::Json).unwrap_err();
    assert_eq!(json, "Invalid JSON at line 1, column 17");

    let jsonc = parse_text("{\n  // note\n  \"token\": abc\n}", ConfigFormat::Jsonc).unwrap_err();
    assert_eq!(jsonc, "Invalid JSON at line 3, column 12");
    assert_eq!(
        parse_text("{ \"a\": 1, /* c */ }", ConfigFormat::Jsonc),
        Ok(json!({ "a": 1 }))
    );

    assert_eq!(parse_text("  \n", ConfigFormat::Yaml), Ok(serde_json::Value::Null));
}
