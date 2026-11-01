//! `harness-code features`: what the installed server can do. The answer is cached per binary by
//! the shared line probe, an older server (a usage error) counts as one without features, and each
//! command of the app waits for its feature.

use crate::memory_cli_support;
use crate::team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::{CodeFeatures, CodeSupport};
use memory_cli_support::{install, installed_binary, installed_binary_at};
use pretty_assertions::assert_eq;
use team_support::{Cli, exits, wait};

fn listing(stdout: &str) -> Cli {
    Cli::default().starting(&["features"], exits(0, stdout, ""))
}

#[test]
fn each_feature_is_read_from_its_own_line_and_the_answer_is_asked_once() {
    let (_dir, env) = installed_binary("harness-code");
    let probe = CodeFeatures::new();
    let cli = listing("code-index\n Code-Prepare \ncode-hint\ncode-match\nsomething-newer\n");

    let answer = wait(probe.support(&env, &cli));

    assert_eq!(
        answer,
        CodeSupport::Listed {
            index: true,
            prepare: true
        }
    );
    assert_eq!(wait(probe.support(&env, &cli)), answer);
    assert_eq!(cli.commands(), [["features"]], "the answer is cached");

    let index_only = wait(CodeFeatures::new().support(&env, &listing("code-index\n")));
    assert!(index_only.index() && !index_only.prepare());

    let hint_only = wait(CodeFeatures::new().support(&env, &listing("code-hint\n")));
    assert!(matches!(hint_only, CodeSupport::Listed { .. }));
    assert!(!hint_only.index() && !hint_only.prepare());
}

#[test]
fn a_server_that_cannot_say_has_no_features() {
    let (_dir, env) = installed_binary("harness-code");

    // No rule answers `features`: a usage error, exit 2, as an older server would.
    let older = wait(CodeFeatures::new().support(&env, &Cli::default()));
    assert_eq!(older, CodeSupport::Unsupported);

    let absent = tempfile::tempdir().unwrap();
    let missing =
        wait(CodeFeatures::new().support(&harness_business::env::Env::for_tests(absent.path()), &listing("")));
    assert_eq!(missing, CodeSupport::NotInstalled);

    for other in [older, missing, CodeSupport::Unknown] {
        assert!(!other.index() && !other.prepare(), "{other:?}");
    }
}

#[test]
fn a_command_waits_for_its_feature() {
    let dir = tempfile::tempdir().unwrap();
    let (env, path) = installed_binary_at(dir.path(), "harness-code");
    install(&path, None);
    let probe = CodeFeatures::new();
    let cli = listing("code-index\n");

    let index = wait(probe.feature_server(&env, &cli, CodeSupport::index, "the code index"));
    assert_eq!(index.unwrap(), path);

    let prepare = wait(probe.feature_server(&env, &cli, CodeSupport::prepare, "preparing the code index"));
    assert!(
        matches!(prepare, Err(BusinessError::Unsupported(message)) if message.contains("harness-code predates preparing the code index")),
    );
}
