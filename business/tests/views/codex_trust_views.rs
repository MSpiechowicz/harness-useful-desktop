//! The Codex mode and trust lookup over small homes of their own: the configured mode follows the
//! user and project layers, and the project layer applies only in a folder Codex trusts (the
//! physical folder itself or the root of its git repository, a worktree's through its common git
//! folder).

use crate::codex_views::write;

use std::path::{Path, PathBuf};

use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harnesses::Codex;
use harness_business::model::{ApprovalMode, NativeApprovalMode};
use pretty_assertions::assert_eq;

#[test]
fn the_configured_mode_follows_the_layers_and_the_profile() {
    let home = tempfile::tempdir().unwrap();
    let project = real(home.path()).join("work/app");
    std::fs::create_dir_all(project.join(".git")).unwrap();
    let env = Env::for_tests(home.path());

    assert_eq!(
        Codex.native_approval_mode(&env, None),
        Some(NativeApprovalMode {
            name: "on-request / workspace-write".to_owned(),
            unattended: false
        })
    );
    assert_eq!(Codex.configured_approval_mode(&env, None), Some(ApprovalMode::Write));

    write(
        &home.path().join(".codex/config.toml"),
        &format!(
            "approval_policy = \"untrusted\"\nsandbox_mode = \"workspace-write\"\n\n{}",
            trust_entry(&project)
        ),
    );
    assert_eq!(
        Codex.configured_approval_mode(&env, None),
        Some(ApprovalMode::AlwaysAsk)
    );

    // A trusted project's settings apply, and a profile they select wins.
    write(
        &project.join(".codex/config.toml"),
        "profile = \"fast\"\n\n[profiles.fast]\napproval_policy = \"never\"\nsandbox_mode = \"danger-full-access\"\n",
    );
    assert_eq!(
        Codex.native_approval_mode(&env, Some(&project)),
        Some(NativeApprovalMode {
            name: "never / danger-full-access".to_owned(),
            unattended: true
        })
    );
    assert_eq!(
        Codex.configured_approval_mode(&env, Some(&project)),
        Some(ApprovalMode::Yolo)
    );

    // The user's file alone is still the user's mode.
    assert_eq!(
        Codex.configured_approval_mode(&env, None),
        Some(ApprovalMode::AlwaysAsk)
    );
}

#[test]
fn an_untrusted_project_cannot_change_the_mode() {
    let home = tempfile::tempdir().unwrap();
    let project = home.path().join("work/app");
    std::fs::create_dir_all(project.join(".git")).unwrap();
    write(&project.join(".codex/config.toml"), "approval_policy = \"never\"\n");
    let env = Env::for_tests(home.path());

    let native = Codex.native_approval_mode(&env, Some(&project)).unwrap();
    assert!(!native.unattended, "{native:?}");
}

#[test]
fn an_unset_mode_is_read_only_in_an_untrusted_folder() {
    let home = tempfile::tempdir().unwrap();
    let project = real(home.path()).join("work/app");
    std::fs::create_dir_all(&project).unwrap();
    let env = Env::for_tests(home.path());

    let native = Codex.native_approval_mode(&env, Some(&project)).unwrap();
    assert_eq!(native.name, "on-request / read-only");
    assert_eq!(Codex.configured_approval_mode(&env, Some(&project)), None);

    write(&home.path().join(".codex/config.toml"), &trust_entry(&project));
    assert_eq!(
        Codex.configured_approval_mode(&env, Some(&project)),
        Some(ApprovalMode::Write)
    );
}

/// The `config.toml` table trusting `folder`. The key is a literal string: a Windows path has
/// backslashes, which a basic string would read as escapes.
pub(crate) fn trust_entry(folder: &Path) -> String {
    // The physical folder, as Codex keys trust: Windows temp folders are 8.3 short names
    // (C:\Users\RUNNER~1\…) and macOS ones sit behind /var.
    let folder = dunce::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf());
    format!("[projects.'{}']\ntrust_level = \"trusted\"\n", folder.display())
}

/// The user's `config.toml` under `home` trusting `folder`.
fn trust(home: &Path, folder: &Path) {
    write(&home.join(".codex/config.toml"), &trust_entry(folder));
}

/// Whether the project's own `approval_policy = "never"` applies (only in a trusted folder).
fn project_layer_applies(env: &Env, project: &Path) -> bool {
    write(&project.join(".codex/config.toml"), "approval_policy = \"never\"\n");

    Codex.native_approval_mode(env, Some(project)).unwrap().unattended
}

#[test]
fn trust_covers_the_folder_and_its_repository_root_only() {
    let home = tempfile::tempdir().unwrap();
    let root = real(home.path());
    let env = Env::for_tests(home.path());

    // A subfolder of a trusted folder that is not a repository is not trusted.
    let plain = root.join("plain");
    trust(home.path(), &plain);
    assert!(!project_layer_applies(&env, &plain.join("sub")));
    assert!(project_layer_applies(&env, &plain));

    // A subfolder of a trusted repository root is.
    let repo = root.join("repo");
    std::fs::create_dir_all(repo.join(".git/worktrees/wt")).unwrap();
    trust(home.path(), &repo);
    assert!(project_layer_applies(&env, &repo.join("crates/app")));

    // So is a worktree of it, through the repository's common git folder.
    let worktree = root.join("wt");
    write(&repo.join(".git/worktrees/wt/commondir"), "../..\n");
    write(
        &worktree.join(".git"),
        &format!("gitdir: {}\n", repo.join(".git/worktrees/wt").display()),
    );
    assert!(project_layer_applies(&env, &worktree));

    // A worktree whose git folder is gone has no repository root.
    std::fs::remove_dir_all(repo.join(".git/worktrees/wt")).unwrap();
    assert!(!project_layer_applies(&env, &worktree));
}

/// The physical path of `path` (a temporary folder may sit below a symlink, as on macOS).
pub(crate) fn real(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap()
}

#[cfg(unix)]
#[test]
fn trust_is_looked_up_for_the_physical_folder() {
    let home = tempfile::tempdir().unwrap();
    let root = real(home.path());
    let env = Env::for_tests(home.path());
    let repo = root.join("data/repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(repo.join("app")).unwrap();
    let link = root.join("work");
    symlink(&root.join("data"), &link);

    // Codex sees the folder a symlink leads to, never the path through it.
    let through_link = format!(
        "[projects.'{}']\ntrust_level = \"trusted\"\n",
        link.join("repo").display()
    );
    write(&home.path().join(".codex/config.toml"), &through_link);
    assert!(!project_layer_applies(&env, &link.join("repo/app")));
    assert!(!project_layer_applies(&env, &link.join("repo")));

    trust(home.path(), &repo);
    assert!(project_layer_applies(&env, &link.join("repo")), "the folder itself");
    assert!(
        project_layer_applies(&env, &link.join("repo/app")),
        "through its repository root"
    );

    // The plain folder of a repository that is itself reached through a symlink.
    trust(home.path(), &repo.join("app"));
    let app_link = root.join("app-link");
    symlink(&repo.join("app"), &app_link);
    assert!(project_layer_applies(&env, &app_link));
}

#[test]
fn a_missing_folder_is_not_trusted() {
    let home = tempfile::tempdir().unwrap();
    let env = Env::for_tests(home.path());
    let missing = real(home.path()).join("gone");
    trust(home.path(), &missing);

    let native = Codex.native_approval_mode(&env, Some(&missing)).unwrap();

    assert_eq!(native.name, "on-request / read-only");
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}
