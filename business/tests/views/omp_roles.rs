//! Mapping the Harness Skills model roles in OMP's settings, and listing the unmapped ones.

use crate::common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{env_at, write_file};
use harness_business::BusinessError;
use harness_business::harness::Harness;
use harness_business::harnesses::Omp;
use harness_business::harnesses::omp::{map_role, map_role_with};
use harness_business::model::UnmappedRole;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const ROLES: &str = r#"{"version": 1, "roles": ["research", "plan", "assess"], "agents": {"planner": "plan", "scout": "research", "release-assessor": "assess", "assessor-helper": "assess"}, "inheritWhenAbsent": ["assess"]}"#;

const SELECTOR: &str = "openai-codex/gpt-6.1-sol:high";

const SETTINGS: &str = "# my OMP settings\ntheme:\n  dark: titanium\n\nmodelRoles:\n  # the roles\n  research: openai-codex/gpt-6-luna:xhigh\n  plan: openai-codex/gpt-6-luna:high   # planning\n  default: opencode-go/longcat\n\n# agents\ntask:\n  agentModelOverrides:\n    scout: \"@research\"\n    planner: \"@plan\"\n  maxConcurrency: 4\nsetupVersion: 2\n";

struct Home {
    dir: TempDir,
}

impl Home {
    fn new(settings: &str, roles: Option<&str>) -> Self {
        let home = Self {
            dir: TempDir::new().unwrap(),
        };

        write_file(&home.settings(), settings);

        if let Some(roles) = roles {
            write_file(&home.agent().join("harness-useful-skills/roles.json"), roles);
        }

        home
    }

    fn agent(&self) -> PathBuf {
        self.dir.path().join(".omp/agent")
    }

    fn settings(&self) -> PathBuf {
        self.agent().join("config.yml")
    }

    fn text(&self) -> String {
        fs::read_to_string(self.settings()).unwrap()
    }

    fn map(&self, role: &str, selector: &str) -> Result<(), BusinessError> {
        map_role(&env_at(self.dir.path()), role, selector)
    }

    fn unmapped(&self) -> Vec<UnmappedRole> {
        Omp.config_view(&env_at(self.dir.path()), None).unwrap().unmapped_roles
    }

    fn leftovers(&self) -> Vec<String> {
        fs::read_dir(self.agent())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".tmp-"))
            .collect()
    }
}

fn unmapped(role: &str, inherits: bool) -> UnmappedRole {
    UnmappedRole {
        role: role.to_owned(),
        inherits_session_model: inherits,
    }
}

#[test]
fn the_view_lists_the_roles_the_file_does_not_map() {
    let home = Home::new(SETTINGS, Some(ROLES));

    assert_eq!(home.unmapped(), [unmapped("assess", true)]);
}

#[test]
fn without_a_roles_file_nothing_is_offered() {
    let home = Home::new(SETTINGS, None);

    assert_eq!(home.unmapped(), []);
    assert!(matches!(
        home.map("assess", SELECTOR),
        Err(BusinessError::Unsupported(_))
    ));
    assert_eq!(home.text(), SETTINGS);
}

#[test]
fn a_roles_file_that_breaks_the_schema_is_ignored() {
    let too_big = format!(
        r#"{{"version": 1, "roles": ["assess"], "agents": {{}}, "pad": "{}"}}"#,
        "x".repeat(20_000)
    );

    for bad in [
        r#"{"version": 2, "roles": ["assess"], "agents": {}}"#,
        r#"{"version": 1, "roles": ["Assess"], "agents": {}}"#,
        r#"{"version": 1, "roles": ["assess"], "agents": {"../x": "assess"}}"#,
        r#"{"version": 1, "roles": ["assess"], "agents": {"scout": "other"}}"#,
        r#"{"version": 1, "roles": ["assess", "assess"], "agents": {}}"#,
        r#"{"version": 1, "roles": ["assess"], "agents": {}, "inheritWhenAbsent": ["other"]}"#,
        "[]",
        "not json",
        too_big.as_str(),
    ] {
        let home = Home::new(SETTINGS, Some(bad));

        assert_eq!(home.unmapped(), [], "{bad}");
        assert!(home.map("assess", SELECTOR).is_err(), "{bad}");
        assert_eq!(home.text(), SETTINGS, "{bad}");
    }
}

#[test]
fn mapping_adds_the_role_and_the_overrides_and_keeps_every_other_byte() {
    let home = Home::new(SETTINGS, Some(ROLES));

    home.map("assess", SELECTOR).unwrap();

    let expected = SETTINGS
        .replace(
            "  default: opencode-go/longcat\n",
            "  default: opencode-go/longcat\n  assess: openai-codex/gpt-6.1-sol:high\n",
        )
        .replace(
            "    planner: \"@plan\"\n",
            "    planner: \"@plan\"\n    release-assessor: \"@assess\"\n    assessor-helper: \"@assess\"\n",
        );

    assert_eq!(home.text(), expected);
    assert_eq!(home.unmapped(), []);
    assert_eq!(home.leftovers(), Vec::<String>::new());
}

#[test]
fn an_override_the_user_has_is_never_changed() {
    let settings = SETTINGS.replace(
        "    planner: \"@plan\"\n",
        "    planner: \"@plan\"\n    release-assessor: \"@research\"\n",
    );
    let home = Home::new(&settings, Some(ROLES));

    home.map("assess", SELECTOR).unwrap();

    let expected = settings
        .replace(
            "  default: opencode-go/longcat\n",
            "  default: opencode-go/longcat\n  assess: openai-codex/gpt-6.1-sol:high\n",
        )
        .replace(
            "    release-assessor: \"@research\"\n",
            "    release-assessor: \"@research\"\n    assessor-helper: \"@assess\"\n",
        );

    assert_eq!(home.text(), expected);
}

#[test]
fn a_role_that_is_mapped_is_refused() {
    let home = Home::new(SETTINGS, Some(ROLES));

    let error = home.map("plan", SELECTOR).unwrap_err();

    assert!(matches!(error, BusinessError::Conflict(_)));
    assert_eq!(home.text(), SETTINGS);
}

#[test]
fn missing_blocks_are_created_at_the_end_without_adding_a_final_newline() {
    let settings = "theme:\n  dark: titanium";
    let home = Home::new(settings, Some(ROLES));

    home.map("assess", SELECTOR).unwrap();

    assert_eq!(
        home.text(),
        "theme:\n  dark: titanium\nmodelRoles:\n  assess: openai-codex/gpt-6.1-sol:high\ntask:\n  agentModelOverrides:\n    release-assessor: \"@assess\"\n    assessor-helper: \"@assess\""
    );
}

#[test]
fn a_missing_overrides_block_goes_into_the_task_block() {
    let settings = "modelRoles:\n  default: a/b\ntask:\n  maxConcurrency: 4\n# end\n";
    let home = Home::new(settings, Some(ROLES));

    home.map("assess", SELECTOR).unwrap();

    assert_eq!(
        home.text(),
        "modelRoles:\n  default: a/b\n  assess: openai-codex/gpt-6.1-sol:high\ntask:\n  maxConcurrency: 4\n  agentModelOverrides:\n    release-assessor: \"@assess\"\n    assessor-helper: \"@assess\"\n# end\n"
    );
}

#[test]
fn windows_line_endings_stay() {
    let settings = SETTINGS.replace('\n', "\r\n");
    let home = Home::new(&settings, Some(ROLES));

    home.map("assess", SELECTOR).unwrap();

    let text = home.text();

    assert_eq!(text.matches('\n').count(), text.matches("\r\n").count());
    assert!(text.contains("  assess: openai-codex/gpt-6.1-sol:high\r\n"));
}

#[test]
fn a_layout_that_cannot_be_edited_safely_is_refused() {
    for settings in [
        "modelRoles: {default: a/b}\n",
        "modelRoles:\n  default: a/b\ntask: {}\n",
        "\"modelRoles\":\n  default: a/b\n",
        "modelRoles:\n\tdefault: a/b\n",
    ] {
        let home = Home::new(settings, Some(ROLES));

        assert!(home.map("assess", SELECTOR).is_err(), "{settings}");
        assert_eq!(home.text(), settings);
    }
}

#[test]
fn a_bad_role_or_model_is_refused() {
    let home = Home::new(SETTINGS, Some(ROLES));

    for (role, selector) in [
        ("unknown", SELECTOR),
        ("assess", "no-slash"),
        ("assess", "a/b c"),
        ("assess", "a/b\nevil: 1"),
        ("assess", "a/b:"),
        ("assess", ""),
    ] {
        assert!(
            matches!(home.map(role, selector), Err(BusinessError::InvalidInput(_))),
            "{role} {selector:?}"
        );
    }

    assert_eq!(home.text(), SETTINGS);
}

#[test]
fn a_file_changed_since_it_was_read_is_not_overwritten() {
    let home = Home::new(SETTINGS, Some(ROLES));
    let changed = format!("{SETTINGS}extra: true\n");

    let result = map_role_with(&env_at(home.dir.path()), "assess", SELECTOR, || {
        fs::write(home.settings(), &changed).unwrap();
    });

    assert!(matches!(result, Err(BusinessError::Conflict(_))));
    assert_eq!(home.text(), changed);
    assert_eq!(home.leftovers(), Vec::<String>::new());
}

#[test]
fn a_file_that_does_not_parse_is_refused() {
    let home = Home::new("modelRoles: [unclosed\n", Some(ROLES));

    assert!(matches!(
        home.map("assess", SELECTOR),
        Err(BusinessError::InvalidInput(_))
    ));
}

#[test]
fn a_missing_settings_file_is_not_created() {
    let home = Home::new("", Some(ROLES));
    fs::remove_file(home.settings()).unwrap();

    assert!(matches!(home.map("assess", SELECTOR), Err(BusinessError::NotFound(_))));
    assert!(!Path::new(&home.settings()).exists());
}

#[cfg(unix)]
#[test]
fn a_symlinked_settings_file_stays_a_link() {
    let home = Home::new("", Some(ROLES));
    let real = home.dir.path().join("dotfiles/config.yml");
    write_file(&real, SETTINGS);
    fs::remove_file(home.settings()).unwrap();
    std::os::unix::fs::symlink(&real, home.settings()).unwrap();

    home.map("assess", SELECTOR).unwrap();

    assert!(fs::symlink_metadata(home.settings()).unwrap().file_type().is_symlink());
    assert!(
        fs::read_to_string(&real)
            .unwrap()
            .contains("  assess: openai-codex/gpt-6.1-sol:high\n")
    );
}

#[cfg(unix)]
#[test]
fn the_mode_of_the_settings_file_stays() {
    use std::os::unix::fs::PermissionsExt;

    let home = Home::new(SETTINGS, Some(ROLES));
    fs::set_permissions(home.settings(), fs::Permissions::from_mode(0o644)).unwrap();

    home.map("assess", SELECTOR).unwrap();

    let mode = fs::metadata(home.settings()).unwrap().permissions().mode();

    assert_eq!(mode & 0o777, 0o644);
}

#[cfg(unix)]
#[test]
fn a_hard_linked_settings_file_is_refused_and_left_alone() {
    let home = Home::new(SETTINGS, Some(ROLES));
    let other = home.dir.path().join("linked.yml");
    fs::hard_link(home.settings(), &other).unwrap();

    assert!(matches!(
        home.map("assess", SELECTOR),
        Err(BusinessError::Unsupported(_))
    ));
    assert_eq!(home.text(), SETTINGS);
    assert_eq!(fs::read_to_string(&other).unwrap(), SETTINGS);
    assert_eq!(home.leftovers(), Vec::<String>::new());
}

#[test]
fn two_roles_mapped_at_once_both_land() {
    let roles = r#"{"version": 1, "roles": ["research", "plan", "assess", "review"], "agents": {}}"#;
    let home = Home::new(SETTINGS, Some(roles));
    let root = home.dir.path();

    std::thread::scope(|scope| {
        for role in ["assess", "review"] {
            scope.spawn(move || {
                map_role(&env_at(root), role, SELECTOR).unwrap();
            });
        }
    });

    let text = home.text();

    assert!(text.contains("  assess: openai-codex/gpt-6.1-sol:high\n"), "{text}");
    assert!(text.contains("  review: openai-codex/gpt-6.1-sol:high\n"), "{text}");
}

#[test]
fn a_flow_map_offers_no_mapping_because_it_cannot_be_edited() {
    let home = Home::new("modelRoles: {research: a/b, plan: a/c}\n", Some(ROLES));

    assert_eq!(home.unmapped(), []);
    assert!(home.map("assess", SELECTOR).is_err());
}

#[test]
fn roles_yaml_would_read_as_something_else_are_not_offered() {
    let roles = r#"{"version": 1, "roles": ["assess", "no", "null", "on", "off", "yes", "true"], "agents": {"scout": "no", "judge": "assess"}, "inheritWhenAbsent": ["no", "assess"]}"#;
    let home = Home::new(SETTINGS, Some(roles));

    assert_eq!(home.unmapped(), [unmapped("assess", true)]);

    for role in ["no", "null", "on"] {
        assert!(matches!(home.map(role, SELECTOR), Err(BusinessError::InvalidInput(_))));
    }

    assert_eq!(home.text(), SETTINGS);
}
