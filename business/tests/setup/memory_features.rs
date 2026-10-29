//! `harness-memory features`: what the installed server can do beyond its commands. The answer is
//! cached per binary like `kinds` (the shared line probe), an older server (a usage error) is
//! cached as one without features, and a probe that could not run is not cached. Team notes with
//! local saves need the three note features, team scripts with local saves the three script
//! features, each set apart from the other. Run against a rule-based stand-in.

use crate::memory_cli_support;
use crate::team_support;

use harness_business::ecosystem::{FeaturesSupport, MemoryFeatures};
use harness_business::process::ExecOutput;
use memory_cli_support::installed;
use pretty_assertions::assert_eq;
use team_support::{Cli, exits, wait};

fn features(stdout: &str) -> Cli {
    Cli::default().starting(&["features"], exits(0, stdout, ""))
}

#[test]
fn a_server_that_lists_every_feature_routes_notes_and_scripts_to_both_and_is_asked_once() {
    let (_dir, env) = installed();
    let probe = MemoryFeatures::new();
    let cli = features(
        "notes-remote-local\n Notes-Hide \npush-json\nscripts-remote-local\nscripts-hide\nscripts-push-json\nsomething-newer\n",
    );

    let answer = wait(probe.support(&env, &cli));
    assert_eq!(
        answer,
        FeaturesSupport::Listed {
            notes_remote_local: true,
            notes_hide: true,
            push_json: true,
            scripts_remote_local: true,
            scripts_hide: true,
            scripts_push_json: true,
            save_guards: false,
            embedder: false,
            settings: false,
            notes_lang: false,
            notes_useful: false,
            code_index: false,
        }
    );
    assert!(answer.team_and_local());
    assert!(answer.scripts_team_and_local());

    assert_eq!(wait(probe.support(&env, &cli)), answer);
    assert_eq!(cli.commands(), [["features"]], "the answer is cached");
}

#[test]
fn a_feature_missing_is_no_team_and_local_notes() {
    let (_dir, env) = installed();

    for listed in ["notes-remote-local\nnotes-hide\n", "notes-hide\npush-json\n", ""] {
        let answer = wait(MemoryFeatures::new().support(&env, &features(listed)));

        assert!(matches!(answer, FeaturesSupport::Listed { .. }), "{listed:?}");
        assert!(!answer.team_and_local(), "{listed:?}");
    }
}

#[test]
fn the_note_and_script_features_are_read_apart() {
    let (_dir, env) = installed();

    let notes_only =
        wait(MemoryFeatures::new().support(&env, &features("notes-remote-local\nnotes-hide\npush-json\n")));
    assert!(notes_only.team_and_local());
    assert!(!notes_only.scripts_team_and_local());

    let scripts_only = wait(MemoryFeatures::new().support(
        &env,
        &features("scripts-remote-local\nscripts-hide\nscripts-push-json\n"),
    ));
    assert!(scripts_only.scripts_team_and_local());
    assert!(!scripts_only.team_and_local());

    for listed in [
        "scripts-remote-local\nscripts-hide\n",
        "scripts-hide\nscripts-push-json\n",
        "scripts-remote-local\nscripts-push-json\npush-json\n",
    ] {
        let answer = wait(MemoryFeatures::new().support(&env, &features(listed)));
        assert!(!answer.scripts_team_and_local(), "{listed:?}");
    }

    for other in [
        FeaturesSupport::Unsupported,
        FeaturesSupport::NotInstalled,
        FeaturesSupport::Unknown,
    ] {
        assert!(!other.scripts_team_and_local(), "{other:?}");
    }
}

#[test]
fn an_older_server_is_cached_and_a_probe_that_did_not_run_is_asked_again() {
    let (_dir, env) = installed();

    let probe = MemoryFeatures::new();
    let older = Cli::default().starting(&["features"], exits(2, "", "usage: harness-memory [mcp | check | …]"));
    assert_eq!(wait(probe.support(&env, &older)), FeaturesSupport::Unsupported);
    assert_eq!(wait(probe.support(&env, &older)), FeaturesSupport::Unsupported);
    assert_eq!(older.commands().len(), 1);
    assert!(!FeaturesSupport::Unsupported.team_and_local());

    let probe = MemoryFeatures::new();
    let slow = Cli::default().starting(
        &["features"],
        ExecOutput {
            timed_out: true,
            ..ExecOutput::default()
        },
    );
    assert_eq!(wait(probe.support(&env, &slow)), FeaturesSupport::Unknown);
    assert_eq!(wait(probe.support(&env, &slow)), FeaturesSupport::Unknown);
    assert_eq!(slow.commands().len(), 2, "not cached");

    let current = features("notes-remote-local\nnotes-hide\npush-json\n");
    assert!(wait(probe.support(&env, &current)).team_and_local());
}

#[test]
fn without_harness_memory_there_are_no_features() {
    let dir = tempfile::tempdir().unwrap();
    let env = harness_business::env::Env::for_tests(dir.path());
    let cli = Cli::default();

    assert_eq!(
        wait(MemoryFeatures::new().support(&env, &cli)),
        FeaturesSupport::NotInstalled
    );
    assert!(cli.commands().is_empty());
}

#[test]
fn save_guards_are_read_from_their_own_line() {
    let (_dir, env) = installed();

    let guarded = wait(MemoryFeatures::new().support(&env, &features("push-json\nsave-guards\n")));
    assert!(guarded.save_guards());
    assert!(!guarded.team_and_local());

    let older = wait(MemoryFeatures::new().support(&env, &features("notes-remote-local\nnotes-hide\npush-json\n")));
    assert!(!older.save_guards());

    assert!(!FeaturesSupport::Unsupported.save_guards());
    assert!(!FeaturesSupport::Unknown.save_guards());
}

#[test]
fn the_embedder_settings_and_lang_features_are_read_from_their_own_lines() {
    let (_dir, env) = installed();

    let current = wait(MemoryFeatures::new().support(&env, &features("embedder\nsettings\nnotes-lang\n")));
    assert!(current.embedder());
    assert!(current.settings());
    assert!(current.notes_lang());
    assert!(!current.save_guards());

    let partial = wait(MemoryFeatures::new().support(&env, &features("settings\n")));
    assert!(!partial.embedder());
    assert!(partial.settings());
    assert!(!partial.notes_lang());

    // An older server lists none of them, so nothing new happens.
    let older = wait(MemoryFeatures::new().support(
        &env,
        &features("notes-remote-local\nnotes-hide\npush-json\nsave-guards\n"),
    ));
    assert!(matches!(older, FeaturesSupport::Listed { .. }));
    assert!(!older.embedder() && !older.settings() && !older.notes_lang());

    for other in [
        FeaturesSupport::Unsupported,
        FeaturesSupport::NotInstalled,
        FeaturesSupport::Unknown,
    ] {
        assert!(
            !other.embedder() && !other.settings() && !other.notes_lang(),
            "{other:?}"
        );
    }
}

#[test]
fn the_code_index_is_read_from_its_own_line() {
    let (_dir, env) = installed();

    let current = wait(MemoryFeatures::new().support(&env, &features("embedder\n Code-Index \n")));
    assert!(current.code_index());

    let older = wait(MemoryFeatures::new().support(&env, &features("embedder\nsettings\nnotes-useful\n")));
    assert!(matches!(older, FeaturesSupport::Listed { .. }));
    assert!(!older.code_index());

    for other in [
        FeaturesSupport::Unsupported,
        FeaturesSupport::NotInstalled,
        FeaturesSupport::Unknown,
    ] {
        assert!(!other.code_index(), "{other:?}");
    }
}
