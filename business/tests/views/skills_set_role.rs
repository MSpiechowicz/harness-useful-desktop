//! Changing the model of a Harness Skills role in `models.yml` of Claude Code, OpenCode and Codex.

use crate::common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{env_at, write_file};
use harness_business::BusinessError;
use harness_business::harnesses::{set_skills_role, set_skills_role_with};
use harness_business::model::{ConfigLayer, HarnessId};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const CLAUDE: &str = "# Useful Skills model roles for Claude Code.\nmodelRoles:\n  research: claude-haiku-5-5:high\n  plan: claude-opus-5-5:high   # planning\n  assess: inherit\n  default: \"claude-opus-5-5:medium\"\n";

struct Setup {
    dir: TempDir,
}

impl Setup {
    fn new(harness: HarnessId, text: &str) -> Self {
        let setup = Self {
            dir: TempDir::new().unwrap(),
        };

        write_file(&setup.models(harness), text);

        setup
    }

    fn scope(&self, harness: HarnessId) -> PathBuf {
        let home = self.dir.path();

        match harness {
            HarnessId::Claude => home.join(".claude"),
            HarnessId::Codex => home.join(".codex"),
            _ => home.join(".config/opencode"),
        }
    }

    fn models(&self, harness: HarnessId) -> PathBuf {
        self.scope(harness).join("harness-useful-skills/models.yml")
    }

    fn text(&self, harness: HarnessId) -> String {
        fs::read_to_string(self.models(harness)).unwrap()
    }

    fn set(&self, harness: HarnessId, role: &str, selector: &str) -> Result<(), BusinessError> {
        set_skills_role(
            &env_at(self.dir.path()),
            harness,
            ConfigLayer::User,
            None,
            role,
            selector,
        )
    }

    fn leftovers(&self, harness: HarnessId) -> Vec<String> {
        fs::read_dir(self.models(harness).parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".tmp-"))
            .collect()
    }
}

#[test]
fn only_the_value_of_the_role_changes() {
    let setup = Setup::new(HarnessId::Claude, CLAUDE);

    setup.set(HarnessId::Claude, "assess", "sonnet:medium").unwrap();
    setup.set(HarnessId::Claude, "plan", "opus").unwrap();
    setup
        .set(HarnessId::Claude, "default", "anthropic/claude-sonnet-5-5:low")
        .unwrap();

    assert_eq!(
        setup.text(HarnessId::Claude),
        CLAUDE
            .replace("assess: inherit", "assess: sonnet:medium")
            .replace("plan: claude-opus-5-5:high   # planning", "plan: opus   # planning")
            .replace("\"claude-opus-5-5:medium\"", "\"anthropic/claude-sonnet-5-5:low\"")
    );
    assert_eq!(setup.leftovers(HarnessId::Claude), Vec::<String>::new());
}

#[test]
fn the_line_ending_quotes_and_missing_final_newline_stay() {
    let text = "modelRoles:\r\n  research: 'gpt-5.5:high'\r\n  plan: inherit";
    let setup = Setup::new(HarnessId::Codex, text);

    setup.set(HarnessId::Codex, "plan", "gpt-5.5:xhigh").unwrap();
    setup.set(HarnessId::Codex, "research", "inherit:low").unwrap();

    assert_eq!(
        setup.text(HarnessId::Codex),
        "modelRoles:\r\n  research: 'inherit:low'\r\n  plan: gpt-5.5:xhigh"
    );
}

#[test]
fn a_role_without_a_row_is_not_added() {
    let setup = Setup::new(HarnessId::Claude, CLAUDE);

    assert!(matches!(
        setup.set(HarnessId::Claude, "security", "opus"),
        Err(BusinessError::NotFound(_))
    ));
    assert!(matches!(
        setup.set(HarnessId::Claude, "Plan", "opus"),
        Err(BusinessError::InvalidInput(_))
    ));
    assert_eq!(setup.text(HarnessId::Claude), CLAUDE);
}

#[test]
fn a_missing_file_is_not_created() {
    let setup = Setup::new(HarnessId::Claude, CLAUDE);

    assert!(matches!(
        setup.set(HarnessId::Codex, "plan", "gpt-5.5"),
        Err(BusinessError::NotFound(_))
    ));
    assert!(!setup.models(HarnessId::Codex).exists());
}

#[test]
fn each_harness_takes_only_the_selectors_of_its_own_parser() {
    let valid: [(HarnessId, &[&str]); 3] = [
        (
            HarnessId::Claude,
            &[
                "opus",
                "inherit",
                "fable:max",
                "anthropic/sonnet:low",
                "claude-opus-5-5:xhigh",
            ],
        ),
        (
            HarnessId::OpenCode,
            &[
                "inherit",
                "anthropic/claude-sonnet-4-5:high",
                "ollama/llama3:8b",
                "openai/gpt-5:none",
            ],
        ),
        (
            HarnessId::Codex,
            &["inherit", "inherit:high", "gpt-5.5", "gpt-5.5:none", "gpt-5.5:xhigh"],
        ),
    ];
    let invalid: [(HarnessId, &[&str]); 3] = [
        (
            HarnessId::Claude,
            &[
                "",
                "gpt-5.5",
                "opus:",
                "opus:none",
                "opus:high:low",
                "Opus",
                "openai/opus",
                "opus high",
                "'opus'",
            ],
        ),
        (
            HarnessId::OpenCode,
            &[
                "",
                "opus",
                "/gpt-5",
                "openai/",
                "inherit:high",
                "Anthropic/x",
                "openai/gpt 5",
                "openai/gpt-5:",
                "a/\"b\"",
            ],
        ),
        (
            HarnessId::Codex,
            &["", "gpt 5", "gpt-5.5:", "inherit:", ":high", "gpt-5#", "\"gpt-5\""],
        ),
    ];

    for (harness, selectors) in valid {
        let setup = Setup::new(harness, "modelRoles:\n  plan: inherit\n");

        for selector in selectors {
            setup
                .set(harness, "plan", selector)
                .unwrap_or_else(|error| panic!("{harness:?} {selector}: {error}"));

            assert_eq!(setup.text(harness), format!("modelRoles:\n  plan: {selector}\n"));
        }
    }

    for (harness, selectors) in invalid {
        let text = "modelRoles:\n  plan: inherit\n";
        let setup = Setup::new(harness, text);

        for selector in selectors {
            assert!(
                matches!(
                    setup.set(harness, "plan", selector),
                    Err(BusinessError::InvalidInput(_))
                ),
                "{harness:?} {selector:?}"
            );
        }

        assert_eq!(setup.text(harness), text);
    }
}

#[test]
fn a_harness_without_a_models_file_is_refused() {
    let setup = Setup::new(HarnessId::Claude, CLAUDE);

    for harness in [HarnessId::Omp, HarnessId::Pi] {
        assert!(matches!(
            setup.set(harness, "plan", "opus"),
            Err(BusinessError::Unsupported(_))
        ));
    }

    let project = setup.dir.path().join("project");
    fs::create_dir_all(&project).unwrap();

    assert!(matches!(
        set_skills_role(
            &env_at(setup.dir.path()),
            HarnessId::Claude,
            ConfigLayer::Project,
            Some(&project),
            "plan",
            "opus"
        ),
        Err(BusinessError::Unsupported(_))
    ));
}

#[test]
fn a_file_laid_out_in_another_way_is_refused() {
    for text in [
        "modelRoles:\n  plan: inherit\n  plan: opus\n",
        "modelRoles:\n  plan: inherit\nother: true\n",
        "  plan: inherit\n",
    ] {
        let setup = Setup::new(HarnessId::Claude, text);

        assert!(
            matches!(
                setup.set(HarnessId::Claude, "plan", "opus"),
                Err(BusinessError::Unsupported(_))
            ),
            "{text:?}"
        );
        assert_eq!(setup.text(HarnessId::Claude), text);
    }
}

#[test]
fn a_file_changed_since_it_was_read_is_not_overwritten() {
    let setup = Setup::new(HarnessId::Claude, CLAUDE);
    let changed = format!("{CLAUDE}  review: opus\n");

    let result = set_skills_role_with(
        &env_at(setup.dir.path()),
        HarnessId::Claude,
        ConfigLayer::User,
        None,
        "plan",
        "opus",
        || fs::write(setup.models(HarnessId::Claude), &changed).unwrap(),
    );

    assert!(matches!(result, Err(BusinessError::Conflict(_))));
    assert_eq!(setup.text(HarnessId::Claude), changed);
    assert_eq!(setup.leftovers(HarnessId::Claude), Vec::<String>::new());
}

struct Project {
    setup: Setup,
    dir: PathBuf,
}

impl Project {
    fn new() -> Self {
        let setup = Setup::new(HarnessId::Claude, CLAUDE);
        let dir = setup.dir.path().join("work/app");

        fs::create_dir_all(&dir).unwrap();

        Self { setup, dir }
    }

    fn models(&self) -> PathBuf {
        self.dir.join(".opencode/harness-useful-skills/models.yml")
    }

    fn set(&self, project: Option<&Path>) -> Result<(), BusinessError> {
        set_skills_role(
            &env_at(self.setup.dir.path()),
            HarnessId::OpenCode,
            ConfigLayer::Project,
            project,
            "plan",
            "openai/gpt-5:high",
        )
    }
}

#[test]
fn the_project_file_changes_and_the_user_file_does_not() {
    let project = Project::new();
    write_file(&project.models(), "modelRoles:\n  plan: inherit\n");
    write_file(
        &project.setup.models(HarnessId::OpenCode),
        "modelRoles:\n  plan: inherit\n",
    );

    project.set(Some(&project.dir)).unwrap();

    assert_eq!(
        fs::read_to_string(project.models()).unwrap(),
        "modelRoles:\n  plan: openai/gpt-5:high\n"
    );
    assert_eq!(
        project.setup.text(HarnessId::OpenCode),
        "modelRoles:\n  plan: inherit\n"
    );
    assert!(matches!(project.set(None), Err(BusinessError::InvalidInput(_))));
}

#[cfg(unix)]
#[test]
fn a_project_file_that_resolves_outside_the_project_is_refused() {
    let project = Project::new();
    let outside = project.setup.dir.path().join("elsewhere/models.yml");
    write_file(&outside, "modelRoles:\n  plan: inherit\n");
    fs::create_dir_all(project.models().parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&outside, project.models()).unwrap();

    assert!(matches!(
        project.set(Some(&project.dir)),
        Err(BusinessError::Forbidden(_))
    ));
    assert_eq!(fs::read_to_string(&outside).unwrap(), "modelRoles:\n  plan: inherit\n");
}

#[cfg(unix)]
#[test]
fn a_user_file_that_is_a_symlink_is_edited_in_place() {
    let setup = Setup::new(HarnessId::Claude, CLAUDE);
    let dotfiles = setup.dir.path().join("dotfiles/models.yml");
    fs::create_dir_all(dotfiles.parent().unwrap()).unwrap();
    fs::rename(setup.models(HarnessId::Claude), &dotfiles).unwrap();
    std::os::unix::fs::symlink(&dotfiles, setup.models(HarnessId::Claude)).unwrap();

    setup.set(HarnessId::Claude, "assess", "haiku").unwrap();

    assert!(
        fs::symlink_metadata(setup.models(HarnessId::Claude))
            .unwrap()
            .is_symlink()
    );
    assert_eq!(
        fs::read_to_string(&dotfiles).unwrap(),
        CLAUDE.replace("assess: inherit", "assess: haiku")
    );
}

#[cfg(unix)]
#[test]
fn the_mode_of_the_file_stays() {
    use std::os::unix::fs::PermissionsExt;

    let setup = Setup::new(HarnessId::Claude, CLAUDE);

    for mode in [0o644, 0o640] {
        fs::set_permissions(setup.models(HarnessId::Claude), fs::Permissions::from_mode(mode)).unwrap();
        setup.set(HarnessId::Claude, "assess", "haiku").unwrap();
        setup.set(HarnessId::Claude, "assess", "inherit").unwrap();

        let kept = fs::metadata(setup.models(HarnessId::Claude))
            .unwrap()
            .permissions()
            .mode();

        assert_eq!(kept & 0o777, mode);
    }
}

#[cfg(unix)]
#[test]
fn a_hard_linked_file_is_refused_and_left_alone() {
    let setup = Setup::new(HarnessId::Claude, CLAUDE);
    let other = setup.dir.path().join("other-name.yml");
    fs::hard_link(setup.models(HarnessId::Claude), &other).unwrap();

    assert!(matches!(
        setup.set(HarnessId::Claude, "assess", "haiku"),
        Err(BusinessError::Unsupported(_))
    ));
    assert_eq!(setup.text(HarnessId::Claude), CLAUDE);
    assert_eq!(fs::read_to_string(&other).unwrap(), CLAUDE);
    assert_eq!(setup.leftovers(HarnessId::Claude), Vec::<String>::new());
}

#[cfg(unix)]
fn set_in_project_with(project: &Project, before_write: impl FnOnce()) -> Result<(), BusinessError> {
    set_skills_role_with(
        &env_at(project.setup.dir.path()),
        HarnessId::OpenCode,
        ConfigLayer::Project,
        Some(&project.dir),
        "plan",
        "openai/gpt-5:high",
        before_write,
    )
}

#[cfg(unix)]
#[test]
fn a_folder_swapped_for_a_link_out_of_the_project_while_editing_is_refused() {
    let project = Project::new();
    write_file(&project.models(), "modelRoles:\n  plan: inherit\n");

    let outside = project.setup.dir.path().join("elsewhere");
    let outside_models = outside.join("harness-useful-skills/models.yml");
    write_file(&outside_models, "modelRoles:\n  plan: inherit\n");

    let config_dir = project.dir.join(".opencode");
    let moved = project.dir.join(".opencode-moved");

    let result = set_in_project_with(&project, || {
        fs::rename(&config_dir, &moved).unwrap();
        std::os::unix::fs::symlink(&outside, &config_dir).unwrap();
    });

    assert!(matches!(result, Err(BusinessError::Forbidden(_))));
    assert_eq!(
        fs::read_to_string(&outside_models).unwrap(),
        "modelRoles:\n  plan: inherit\n"
    );
}

#[cfg(unix)]
#[test]
fn a_file_swapped_for_a_link_out_of_the_project_while_editing_is_refused() {
    let project = Project::new();
    write_file(&project.models(), "modelRoles:\n  plan: inherit\n");

    let outside = project.setup.dir.path().join("elsewhere/models.yml");
    write_file(&outside, "modelRoles:\n  plan: inherit\n");

    let result = set_in_project_with(&project, || {
        fs::remove_file(project.models()).unwrap();
        std::os::unix::fs::symlink(&outside, project.models()).unwrap();
    });

    assert!(matches!(result, Err(BusinessError::Forbidden(_))));
    assert_eq!(fs::read_to_string(&outside).unwrap(), "modelRoles:\n  plan: inherit\n");
}

#[test]
fn a_missing_project_file_is_not_found() {
    let project = Project::new();

    assert!(matches!(
        project.set(Some(&project.dir)),
        Err(BusinessError::NotFound(_))
    ));
}
