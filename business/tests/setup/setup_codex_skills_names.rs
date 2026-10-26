//! The Harness Skills for Codex found under the names before Harness Skills 0.4.0 (folder
//! `useful-skills`): installed, but the installer is offered to migrate them, and a record under the
//! new name decides alone.

use crate::common::write_file as write;
use crate::setup_codex_skills::{codex_skills, env_with_node, legacy_record, script_of, skills_record};

use harness_business::ecosystem::catalog::CODEX_SKILLS_INSTALL_SH;
use harness_business::env::OsKind;
use harness_business::model::SkillsSetup;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn an_install_under_the_old_names_counts_but_is_updated_by_the_installer() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_node(home.path()).with_os(OsKind::Linux);

    // Even a version that supports Codex is migrated: only the installer renames the install.
    write(&legacy_record(home.path()), r#"{"version":"0.3.14"}"#);

    let (command, side_effects, outdated_from) = script_of(codex_skills(&env).await);

    assert_eq!(command, CODEX_SKILLS_INSTALL_SH);
    assert_eq!(outdated_from.as_deref(), Some("0.3.14"));
    assert!(
        side_effects.iter().any(|line| line.contains("harness-useful-skills")),
        "the migration is announced: {side_effects:?}"
    );
    assert!(side_effects.iter().all(|line| !line.contains(';')), "{side_effects:?}");
}

#[tokio::test]
async fn a_record_under_the_new_name_decides_alone() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_node(home.path());

    // Both folders (a leftover of the old install): the new record counts, nothing is offered.
    write(&legacy_record(home.path()), r#"{"version":"0.3.14"}"#);
    write(&skills_record(home.path()), r#"{"version":"0.4.0"}"#);

    assert_eq!(
        codex_skills(&env).await,
        SkillsSetup::Installed {
            version: Some("0.4.0".to_owned())
        }
    );

    // An unusable new record is not replaced by the old one.
    write(&skills_record(home.path()), "not json");
    let (_, _, outdated_from) = script_of(codex_skills(&env).await);
    assert_eq!(outdated_from, None);
}
