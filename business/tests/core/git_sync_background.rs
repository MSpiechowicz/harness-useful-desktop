//! The background fetch nobody clicked runs no program the repository names, no hook and no
//! `ext::` transport, while the clicked fetch still runs them like a terminal would.
#![cfg(unix)]

use crate::git_support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use git_support::{BASE_CONFIG, WithPath, World, world};
use harness_business::git::git_fetch;
use pretty_assertions::assert_eq;

/// Pushes a commit from another clone, so a fetch of `work` has something to bring.
fn push_from_other(world: &World, name: &str) {
    let mut other = if world.home.home.join("other").exists() {
        world.home.clone_of(&world.remote, &format!("other-{name}"))
    } else {
        world.other()
    };
    other.commit_file(&format!("{name}.txt"), "x\n");
    other.git(&["pull", "-q", "--rebase", "origin", "main"]);
    other.git(&["push", "-q", "origin", "main"]);
}

fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap(); // guard-allow: installer-shell
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A background fetch of `work` skips it without running what `marker` records and brings nothing;
/// a clicked fetch runs it and brings the one commit pushed before.
async fn assert_skipped_in_background_but_run_on_click(world: &World, marker: &Path, what: &str) {
    let (home, work) = (&world.home, &world.work);
    let skipped = git_fetch(&home.env, &home.runner, &work.path, None, true)
        .await
        .unwrap();

    assert!(!marker.exists(), "the background fetch ran {what}");
    assert_eq!(skipped.behind, 0, "nothing was fetched");

    let clicked = git_fetch(&home.env, &home.runner, &work.path, None, false)
        .await
        .unwrap();

    assert!(marker.exists(), "a clicked fetch runs it, as a terminal would");
    assert_eq!(clicked.behind, 1);
}

#[tokio::test]
async fn a_background_fetch_skips_a_repository_that_names_a_program_but_a_click_runs_it() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    let marker = home.home.join("uploadpack.marker");
    let wrapper = home.home.join("upload-pack.sh");
    script(
        &wrapper,
        &format!("echo ran >> '{}'\nexec git upload-pack \"$@\"", marker.display()),
    );
    work.git(&["config", "remote.origin.uploadpack", &wrapper.to_string_lossy()]);
    push_from_other(&world, "one");

    assert_skipped_in_background_but_run_on_click(&world, &marker, "the program").await;
}

#[tokio::test]
async fn every_key_that_runs_a_program_or_rewrites_a_remote_stops_the_background_fetch() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    push_from_other(&world, "one");

    let keys = [
        ("core.sshCommand", "ssh"),
        ("core.askPass", "true"),
        ("credential.helper", "cache"),
        ("credential.https://example.invalid.helper", "cache"),
        ("remote.origin.uploadpack", "git-upload-pack"),
        ("remote.origin.receivepack", "git-receive-pack"),
        ("remote.origin.vcs", "svn"),
        ("url.https://example.invalid/.insteadOf", "elsewhere:"),
        ("url.https://example.invalid/.pushInsteadOf", "elsewhere:"),
        ("protocol.file.allow", "always"),
        ("include.path", "other-config"),
        ("http.proxy", "http://127.0.0.1:9"),
        ("http.https://example.invalid/.sslVerify", "false"),
        ("remote.origin.proxy", "http://127.0.0.1:9"),
        ("remote.origin.proxyAuthMethod", "basic"),
    ];

    for (key, value) in keys {
        work.git(&["config", "--local", key, value]);

        let state = git_fetch(&home.env, &home.runner, &work.path, None, true)
            .await
            .unwrap();
        assert_eq!(state.behind, 0, "{key} did not stop the background fetch");

        work.git(&["config", "--local", "--unset-all", key]);
    }

    let state = git_fetch(&home.env, &home.runner, &work.path, None, true)
        .await
        .unwrap();
    assert_eq!(state.behind, 1, "without them it fetches");
}

#[tokio::test]
async fn a_background_fetch_runs_no_hook_but_a_clicked_one_does() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    let marker = home.home.join("hook.marker");
    work.install_hook(
        "reference-transaction",
        &format!("echo \"$1\" >> '{}'", marker.display()),
    );
    push_from_other(&world, "one");

    let background = git_fetch(&home.env, &home.runner, &work.path, None, true)
        .await
        .unwrap();

    assert_eq!(background.behind, 1, "the refs were updated");
    assert!(!marker.exists(), "the background fetch ran a hook");

    push_from_other(&world, "two");
    git_fetch(&home.env, &home.runner, &work.path, None, false)
        .await
        .unwrap();

    assert!(marker.exists(), "a clicked fetch runs the repository's hooks");
}

#[tokio::test]
async fn a_repository_hooks_path_neither_stops_the_background_fetch_nor_runs_its_hooks() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    let marker = home.home.join("hooks-path.marker");
    let hooks = home.home.join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    script(
        &hooks.join("reference-transaction"),
        &format!("echo \"$1\" >> '{}'", marker.display()),
    );
    work.git(&["config", "--local", "core.hooksPath", &hooks.to_string_lossy()]);
    push_from_other(&world, "one");

    let state = git_fetch(&home.env, &home.runner, &work.path, None, true)
        .await
        .unwrap();

    assert_eq!(state.behind, 1, "a hooks path stopped the background fetch");
    assert!(!marker.exists(), "the background fetch ran a hook from the hooks path");
}

#[tokio::test]
async fn a_background_fetch_runs_no_hook_planted_where_a_relative_null_device_would_point() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    let marker = home.home.join("nul.marker");
    // Windows' `NUL` as a hooks path is read as this directory at the top of the work tree.
    for dir in ["NUL", "dev/null"] {
        let hooks = work.path.join(dir);
        fs::create_dir_all(&hooks).unwrap();
        script(
            &hooks.join("reference-transaction"),
            &format!("echo \"$1\" >> '{}'", marker.display()),
        );
    }
    push_from_other(&world, "one");

    let state = git_fetch(&home.env, &home.runner, &work.path, None, true)
        .await
        .unwrap();

    assert_eq!(state.behind, 1, "the refs were updated");
    assert!(
        !marker.exists(),
        "the background fetch ran a hook planted in the work tree"
    );
}

#[tokio::test]
async fn a_background_fetch_never_starts_an_ext_transport() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    let marker = home.home.join("ext.marker");
    let command = home.home.join("ext.sh");
    script(&command, &format!("echo ran >> '{}'\nexit 1", marker.display()));
    // The user allows `ext::` in the global configuration: only the background fetch overrides it.
    fs::write(
        home.home.join(".gitconfig"),
        format!("{BASE_CONFIG}[user]\n\tname = Demo User\n\temail = demo@example.com\n[protocol \"ext\"]\n\tallow = always\n"),
    )
    .unwrap();
    work.git(&["config", "remote.origin.url", &format!("ext::{}", command.display())]);

    let background = git_fetch(&home.env, &home.runner, &work.path, None, true).await;

    assert!(background.is_err());
    assert!(!marker.exists(), "the background fetch started the transport");

    let clicked = git_fetch(&home.env, &home.runner, &work.path, None, false).await;

    assert!(clicked.is_err(), "the transport fails");
    assert!(marker.exists(), "a clicked fetch starts it, as a terminal would");
}

#[tokio::test]
async fn a_background_fetch_runs_from_every_work_tree_of_a_repository_with_linked_ones() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    let linked = home.home.join("linked");
    work.git(&["worktree", "add", "-q", "-b", "wt", &linked.to_string_lossy()]);
    let linked = dunce::canonicalize(&linked).unwrap();

    for (name, dir) in [("one", &work.path), ("two", &linked)] {
        push_from_other(&world, name);
        let before = work.git(&["rev-parse", "origin/main"]);

        git_fetch(&home.env, &home.runner, dir, None, true).await.unwrap();

        let after = work.git(&["rev-parse", "origin/main"]);
        assert_ne!(before, after, "the background fetch from {} was skipped", dir.display());
    }
}

#[tokio::test]
async fn a_background_fetch_also_reads_the_configuration_of_a_work_tree_that_has_one() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    let linked = home.home.join("linked");
    work.git(&["config", "extensions.worktreeConfig", "true"]);
    work.git(&["worktree", "add", "-q", "-b", "wt", &linked.to_string_lossy()]);
    let linked = dunce::canonicalize(&linked).unwrap();
    home.git_in(&linked, &["config", "--worktree", "core.sshCommand", "ssh"]);
    push_from_other(&world, "one");
    let before = work.git(&["rev-parse", "origin/main"]);

    git_fetch(&home.env, &home.runner, &linked, None, true).await.unwrap();

    assert_eq!(
        before,
        work.git(&["rev-parse", "origin/main"]),
        "its own sshCommand did not stop it"
    );

    git_fetch(&home.env, &home.runner, &work.path, None, true)
        .await
        .unwrap();

    assert_ne!(
        before,
        work.git(&["rev-parse", "origin/main"]),
        "the main work tree sets none"
    );
}

#[tokio::test]
async fn a_background_fetch_runs_no_transport_helper_a_local_url_names() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    let marker = home.home.join("helper.marker");
    let bin = home.home.join("bin");
    fs::create_dir(&bin).unwrap();
    script(
        &bin.join("git-remote-evil"),
        &format!("echo ran >> '{}'\nexit 1", marker.display()),
    );
    let runner = WithPath { dir: bin };
    work.git(&["config", "remote.origin.url", "evil::somewhere"]);

    let background = git_fetch(&home.env, &runner, &work.path, None, true).await;

    assert!(background.is_err());
    assert!(!marker.exists(), "the background fetch ran the helper");

    let clicked = git_fetch(&home.env, &runner, &work.path, None, false).await;

    assert!(clicked.is_err(), "the helper fails");
    assert!(marker.exists(), "a clicked fetch runs it, as a terminal would");
}

#[tokio::test]
async fn a_background_fetch_skips_a_repository_with_a_config_hook_but_a_click_runs_it() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    let marker = home.home.join("config-hook.marker");
    work.git(&[
        "config",
        "hook.x.command",
        &format!("echo ran >> '{}'", marker.display()),
    ]);
    work.git(&["config", "hook.x.event", "reference-transaction"]);
    push_from_other(&world, "one");

    assert_skipped_in_background_but_run_on_click(&world, &marker, "a config hook").await;
}

#[tokio::test]
async fn a_background_fetch_runs_no_global_config_hook_but_a_clicked_one_does() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    let marker = home.home.join("global-hook.marker");
    let config = home.home.join(".gitconfig");
    push_from_other(&world, "one");
    let mut text = fs::read_to_string(&config).unwrap();
    text.push_str(&format!(
        "[hook \"x\"]\n\tcommand = echo ran >> '{}'\n\tevent = reference-transaction\n",
        marker.display()
    ));
    fs::write(&config, text).unwrap();

    let background = git_fetch(&home.env, &home.runner, &work.path, None, true)
        .await
        .unwrap();

    assert_eq!(background.behind, 1, "the refs were updated");
    assert!(!marker.exists(), "the background fetch ran a global config hook");

    // The hook is global: the other clone's own commands run it too, so start from a clean marker.
    push_from_other(&world, "two");
    fs::remove_file(&marker).ok();
    git_fetch(&home.env, &home.runner, &work.path, None, false)
        .await
        .unwrap();

    assert!(marker.exists(), "a clicked fetch runs the user's hooks");
}
