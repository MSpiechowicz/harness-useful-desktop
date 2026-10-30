//! Building the code index before an agent asks: `code build --project P --json` against the
//! stand-in of `fake_agent` (see `src/bin/fake_agent/memory_code.rs`): what it answers the first
//! and the second time, why it skips, and what the app makes of an answer it cannot read.

use std::time::Duration;

use harness_business::BusinessError;
use harness_business::ecosystem::memory_code_prepare::PREPARE_TIMEOUT;
use harness_business::ecosystem::{CodePrepared, FeaturesSupport, prepare_code_index};
use memory_stand_in::{CURRENT, Setup};
use pretty_assertions::assert_eq;

use crate::memory_stand_in;

fn skipped(reason: &str, error: Option<&str>) -> CodePrepared {
    CodePrepared::Skipped {
        reason: reason.to_owned(),
        error: error.map(str::to_owned),
    }
}

#[tokio::test]
async fn a_build_asks_for_the_project_within_the_long_limit_and_says_built_then_indexed() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let first = prepare_code_index(&setup.env, setup.runner(), &setup.binary, &project).await;
    assert_eq!(first.unwrap(), CodePrepared::Built { files: 3 });

    let second = prepare_code_index(&setup.env, setup.runner(), &setup.binary, &project).await;
    assert_eq!(second.unwrap(), skipped("indexed", None));

    let project = project.to_string_lossy().into_owned();
    for run in 0..2 {
        assert_eq!(
            setup.run_args(run),
            ["code", "build", "--project", project.as_str(), "--json"]
        );
    }
    assert_eq!(setup.runs()[0].timeout, PREPARE_TIMEOUT);
    assert!(
        PREPARE_TIMEOUT > Duration::from_secs(10 * 60),
        "longer than the server's own cap"
    );
}

#[tokio::test]
async fn a_build_the_server_skips_says_why() {
    let off = Setup::installed(CURRENT, &[("FAKE_CODE_INDEX", "off")]);
    let answer = prepare_code_index(&off.env, off.runner(), &off.binary, &off.project()).await;
    assert_eq!(answer.unwrap(), skipped("off", None));

    let broken = Setup::installed(CURRENT, &[("FAKE_CODE_BUILD", "error")]);
    let answer = prepare_code_index(&broken.env, broken.runner(), &broken.binary, &broken.project()).await;
    assert_eq!(answer.unwrap(), skipped("error", Some("disk full")));
}

#[tokio::test]
async fn an_answer_the_app_cannot_read_is_a_protocol_error() {
    let garbled: &'static [(&str, &str)] = &[("FAKE_CODE_BUILD", "garbled")];
    let incomplete: &'static [(&str, &str)] = &[("FAKE_CODE_BUILD", "incomplete")];

    for vars in [garbled, incomplete] {
        let setup = Setup::installed(CURRENT, vars);

        let answer = prepare_code_index(&setup.env, setup.runner(), &setup.binary, &setup.project()).await;

        assert!(
            matches!(answer, Err(BusinessError::Protocol(_))),
            "{vars:?}: {answer:?}"
        );
    }
}

#[tokio::test]
async fn a_server_without_code_prepare_is_unsupported_and_a_missing_one_not_found() {
    let older = Setup::installed(CURRENT, &[("FAKE_MEMORY_FEATURES", "code-index")]);
    let refused = older
        .feature_server(FeaturesSupport::code_prepare, "preparing the code index")
        .await;
    assert!(matches!(refused, Err(BusinessError::Unsupported(_))), "{refused:?}");

    let usage = prepare_code_index(&older.env, older.runner(), &older.binary, &older.project()).await;
    assert!(
        matches!(usage, Err(BusinessError::Unsupported(_))),
        "the usage error of an older server: {usage:?}"
    );

    let absent = Setup::without_server(CURRENT, &[]);
    let refused = absent
        .feature_server(FeaturesSupport::code_prepare, "preparing the code index")
        .await;
    assert!(matches!(refused, Err(BusinessError::NotFound(_))), "{refused:?}");
}
