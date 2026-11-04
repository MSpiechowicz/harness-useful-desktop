//! Changing the model of a role that OMP's settings map, in place.

use crate::common;

use std::fs;
use std::path::PathBuf;

use common::{env_at, write_file};
use harness_business::BusinessError;
use harness_business::harnesses::omp::set_role;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const SELECTOR: &str = "openai-codex/gpt-6.1-sol:high";

const SETTINGS: &str = "# my OMP settings\ntheme:\n  dark: titanium\n\nmodelRoles:\n  # the roles\n  research: openai-codex/gpt-6-luna:xhigh\n  plan: \"openai-codex/gpt-6-luna:high\"   # planning\n  planner: 'web/kimi'\n  default: opencode-go/longcat\n\ntask:\n  agentModelOverrides:\n    planner: \"@plan\"\nsetupVersion: 2\n";

struct Home {
    dir: TempDir,
}

impl Home {
    fn new(settings: &str) -> Self {
        let home = Self {
            dir: TempDir::new().unwrap(),
        };

        write_file(&home.settings(), settings);

        home
    }

    fn settings(&self) -> PathBuf {
        self.dir.path().join(".omp/agent/config.yml")
    }

    fn text(&self) -> String {
        fs::read_to_string(self.settings()).unwrap()
    }

    fn set(&self, role: &str, selector: &str) -> Result<(), BusinessError> {
        set_role(&env_at(self.dir.path()), role, selector)
    }
}

#[test]
fn only_the_value_of_a_plain_row_changes() {
    let home = Home::new(SETTINGS);

    home.set("research", SELECTOR).unwrap();

    assert_eq!(
        home.text(),
        SETTINGS.replace(
            "research: openai-codex/gpt-6-luna:xhigh",
            "research: openai-codex/gpt-6.1-sol:high"
        )
    );
}

#[test]
fn quotes_and_a_trailing_comment_stay() {
    let home = Home::new(SETTINGS);

    home.set("plan", SELECTOR).unwrap();
    home.set("planner", "anthropic/claude-haiku-4-5").unwrap();

    assert_eq!(
        home.text(),
        SETTINGS
            .replace(
                "plan: \"openai-codex/gpt-6-luna:high\"   # planning",
                "plan: \"openai-codex/gpt-6.1-sol:high\"   # planning"
            )
            .replace("planner: 'web/kimi'", "planner: 'anthropic/claude-haiku-4-5'")
    );
}

#[test]
fn any_role_the_file_maps_can_change_without_a_skills_roles_file() {
    let home = Home::new(SETTINGS);

    home.set("default", SELECTOR).unwrap();

    assert!(home.text().contains("  default: openai-codex/gpt-6.1-sol:high\n"));
}

#[test]
fn windows_line_endings_and_a_missing_final_newline_stay() {
    let settings = "modelRoles:\r\n  default: opencode-go/longcat\r\n  web: web/kimi";
    let home = Home::new(settings);

    home.set("web", SELECTOR).unwrap();
    home.set("default", "web/kimi").unwrap();

    assert_eq!(
        home.text(),
        "modelRoles:\r\n  default: web/kimi\r\n  web: openai-codex/gpt-6.1-sol:high"
    );
}

#[test]
fn a_role_the_file_does_not_map_is_not_found() {
    let home = Home::new(SETTINGS);

    assert!(matches!(home.set("assess", SELECTOR), Err(BusinessError::NotFound(_))));
    assert_eq!(home.text(), SETTINGS);
}

#[test]
fn a_bad_role_or_model_is_refused() {
    let home = Home::new(SETTINGS);

    for (role, selector) in [
        ("research", "no-provider"),
        ("research", "openai/gpt-5:"),
        ("research", "openai/gpt 5"),
        ("research\nx", SELECTOR),
        ("", SELECTOR),
    ] {
        assert!(
            matches!(home.set(role, selector), Err(BusinessError::InvalidInput(_))),
            "{role:?} {selector:?}"
        );
    }

    assert_eq!(home.text(), SETTINGS);
}

#[test]
fn a_layout_that_cannot_be_edited_safely_is_refused() {
    for settings in [
        "modelRoles: {default: opencode-go/longcat}\n",
        "modelRoles:\n  default: >-\n    opencode-go/longcat\n",
        "modelRoles:\n  default: \"open\\x63ode-go/longcat\"\n",
        "modelRoles:\n  default: &m opencode-go/longcat\n",
        "modelRoles:\n  \"default\": opencode-go/longcat\n",
        "modelRoles:\n  default:\n    nested: opencode-go/longcat\n",
    ] {
        let home = Home::new(settings);

        assert!(home.set("default", SELECTOR).is_err(), "{settings:?}");
        assert_eq!(home.text(), settings);
    }
}

#[test]
fn tabs_around_the_value_and_before_a_comment_stay() {
    let settings = "modelRoles:\n  default:\topencode-go/longcat\t# note\n";
    let home = Home::new(settings);

    home.set("default", SELECTOR).unwrap();

    assert_eq!(
        home.text(),
        "modelRoles:\n  default:\topenai-codex/gpt-6.1-sol:high\t# note\n"
    );
}
