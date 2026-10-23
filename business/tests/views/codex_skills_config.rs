//! The Harness Skills model roles in the Codex config view: `harness-useful-skills/models.yml` of
//! the Codex home, listed only for an install of Harness Skills 0.4.0 or newer under the new
//! names (older installs have no Codex roles) and only when the file exists.

use crate::common;

use std::path::Path;

use common::{env_at, write_file};
use harness_business::harness::Harness;
use harness_business::harnesses::Codex;
use harness_business::model::{ConfigFormat, ConfigLayer, ConfigSection, ConfigValue, ConfigView};
use pretty_assertions::assert_eq;

const MODELS: &str = "user.harnessSkillsModels";

const ROLES: &str = "modelRoles:\n  default: gpt-5.5:medium\n  plan: gpt-5.5:high\n  research: inherit\n";

fn config(home: &Path) -> ConfigView {
    Codex.config_view(&env_at(home), None).unwrap()
}

fn models(view: &ConfigView) -> Option<&ConfigSection> {
    view.sections.iter().find(|section| section.id == MODELS)
}

fn record(home: &Path, folder: &str, version: &str) {
    write_file(
        &home.join(".codex").join(folder).join("install-record.json"),
        &format!(r#"{{"version":"{version}"}}"#),
    );
}

fn roles(home: &Path, folder: &str) {
    write_file(&home.join(".codex").join(folder).join("models.yml"), ROLES);
}

#[test]
fn a_current_install_with_a_models_file_lists_the_roles_in_the_user_layer() {
    let home = tempfile::tempdir().unwrap();
    record(home.path(), "harness-useful-skills", "0.4.0");
    roles(home.path(), "harness-useful-skills");

    let view = config(home.path());
    let section = models(&view).expect("a model roles section");

    assert_eq!(section.title, "Harness Skills model roles");
    assert_eq!(section.source.layer, ConfigLayer::User);
    assert_eq!(section.source.format, ConfigFormat::Yaml);
    assert_eq!(
        Path::new(&section.source.path),
        home.path().join(".codex/harness-useful-skills/models.yml")
    );
    assert_eq!(section.error, None);

    let ConfigValue::Map(rows) = &section.entries[0].value else {
        panic!("modelRoles is a map");
    };
    let rows: Vec<(&str, &ConfigValue)> = rows.iter().map(|row| (row.key.as_str(), &row.value)).collect();

    assert_eq!(rows[0], ("default", &ConfigValue::Text("gpt-5.5:medium".to_owned())));
    assert_eq!(rows[2], ("research", &ConfigValue::Text("inherit".to_owned())));
}

#[test]
fn newer_versions_list_the_roles_too() {
    for version in ["0.4.1", "0.10.0", "1.0.0"] {
        let home = tempfile::tempdir().unwrap();
        record(home.path(), "harness-useful-skills", version);
        roles(home.path(), "harness-useful-skills");

        assert!(models(&config(home.path())).is_some(), "{version}");
    }
}

#[test]
fn an_install_before_0_4_0_has_no_roles_so_a_stray_file_is_not_shown() {
    for version in ["0.3.13", "0.3.99"] {
        let home = tempfile::tempdir().unwrap();
        record(home.path(), "harness-useful-skills", version);
        roles(home.path(), "harness-useful-skills");

        assert!(models(&config(home.path())).is_none(), "{version}");
    }
}

#[test]
fn an_install_under_the_old_names_has_no_roles_either() {
    let home = tempfile::tempdir().unwrap();
    record(home.path(), "useful-skills", "0.3.14");
    roles(home.path(), "useful-skills");

    assert!(models(&config(home.path())).is_none());
}

#[test]
fn nothing_is_listed_without_an_install_or_without_the_file() {
    let home = tempfile::tempdir().unwrap();
    roles(home.path(), "harness-useful-skills");
    assert!(
        models(&config(home.path())).is_none(),
        "a file without an install record"
    );

    let home = tempfile::tempdir().unwrap();
    record(home.path(), "harness-useful-skills", "0.4.0");
    assert!(
        models(&config(home.path())).is_none(),
        "a current install without the file"
    );

    let home = tempfile::tempdir().unwrap();
    write_file(
        &home.path().join(".codex/harness-useful-skills/install-record.json"),
        r#"{"name":"harness-useful-skills"}"#,
    );
    roles(home.path(), "harness-useful-skills");
    assert!(models(&config(home.path())).is_none(), "a record without a version");
}

#[test]
fn the_roles_follow_codex_home() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = home.path().join("elsewhere");
    write_file(
        &elsewhere.join("harness-useful-skills/install-record.json"),
        r#"{"version":"0.4.0"}"#,
    );
    write_file(&elsewhere.join("harness-useful-skills/models.yml"), ROLES);

    let env = env_at(home.path()).with_var("CODEX_HOME", elsewhere.to_string_lossy());
    let view = Codex.config_view(&env, None).unwrap();

    assert_eq!(
        Path::new(&models(&view).expect("the roles of CODEX_HOME").source.path),
        elsewhere.join("harness-useful-skills/models.yml")
    );
}

#[test]
fn an_unparsable_file_is_an_error_section() {
    let home = tempfile::tempdir().unwrap();
    record(home.path(), "harness-useful-skills", "0.4.0");
    write_file(
        &home.path().join(".codex/harness-useful-skills/models.yml"),
        "modelRoles: [oops",
    );

    let view = config(home.path());
    let error = models(&view).and_then(|section| section.error.as_deref());

    assert!(
        error.is_some_and(|error| error.starts_with("Invalid YAML")),
        "{error:?}"
    );
}
