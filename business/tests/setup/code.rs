//! The code graph switch of Harness Code through `status|enable|disable --json` against the
//! stand-in of `fake_agent` (see `src/bin/fake_agent/code.rs`), and what the hooks make of it.

use std::path::PathBuf;

use harness_business::BusinessError;
use harness_business::ecosystem::{
    CodeFeatures, CodeGraphSupport, CodeSupport, code_graph_support, get_code_index, set_code_index,
};
use memory_stand_in::Setup;
use pretty_assertions::assert_eq;

use crate::memory_stand_in;

const CODE: &str = "harness-code";

/// The code server of `setup` when it lists `code-index`, or why there is none.
async fn index_server(setup: &Setup) -> harness_business::Result<PathBuf> {
    CodeFeatures::new()
        .feature_server(&setup.env, setup.runner(), CodeSupport::index, "the code index")
        .await
}

#[tokio::test]
async fn the_switch_round_trips_through_the_server_that_lists_code_index() {
    let setup = Setup::installed_as(CODE, &[], &[]);
    let server = index_server(&setup).await.expect("code-index is listed");

    let first = get_code_index(&setup.env, setup.runner(), &server).await.unwrap();
    assert_eq!((first.enabled, first.source.as_str()), (true, "default"));

    let off = set_code_index(&setup.env, setup.runner(), &server, false)
        .await
        .unwrap();
    assert_eq!((off.enabled, off.source.as_str()), (false, "file"));
    assert!(
        !get_code_index(&setup.env, setup.runner(), &server)
            .await
            .unwrap()
            .enabled
    );

    let on = set_code_index(&setup.env, setup.runner(), &server, true).await.unwrap();
    assert!(on.enabled);

    let commands: Vec<Vec<String>> = (0..setup.runs().len()).map(|run| setup.run_args(run)).collect();
    assert_eq!(
        commands[1..],
        [
            vec!["status", "--json"],
            vec!["disable", "--json"],
            vec!["status", "--json"],
            vec!["enable", "--json"],
        ]
    );
}

async fn support_of(setup: &Setup) -> CodeGraphSupport {
    code_graph_support(&setup.env, setup.runner(), &CodeFeatures::new()).await
}

#[tokio::test]
async fn the_code_graph_of_the_hooks_needs_the_feature_and_follows_the_switch() {
    let on = Setup::installed_as(CODE, &[], &[]);
    assert_eq!(support_of(&on).await, CodeGraphSupport { listed: true, on: true });

    let off = Setup::installed_as(CODE, &[], &[("FAKE_CODE_INDEX", "off")]);
    assert_eq!(
        support_of(&off).await,
        CodeGraphSupport {
            listed: true,
            on: false
        }
    );

    let older = Setup::installed_as(CODE, &[], &[("FAKE_CODE_FEATURES", "code-hint")]);
    assert_eq!(support_of(&older).await, CodeGraphSupport::default());

    let absent = Setup::without_server_as(CODE, &[], &[]);
    assert_eq!(support_of(&absent).await, CodeGraphSupport::default());
}

#[tokio::test]
async fn the_environment_overrides_are_reported_as_their_source() {
    let setup = Setup::installed_as(CODE, &[], &[("FAKE_CODE_INDEX", "off")]);

    let state = get_code_index(&setup.env, setup.runner(), &setup.binary).await.unwrap();

    assert_eq!((state.enabled, state.source.as_str()), (false, "env"));
}

#[tokio::test]
async fn a_server_that_cannot_write_the_choice_answers_with_its_line() {
    let setup = Setup::installed_as(CODE, &[], &[("FAKE_CODE_REFUSE", "1")]);

    let refused = set_code_index(&setup.env, setup.runner(), &setup.binary, false).await;

    match refused {
        Err(BusinessError::Process(line)) => assert!(line.contains("cannot write code.toml"), "{line}"),
        other => panic!("expected the server's line, got {other:?}"),
    }
}

#[tokio::test]
async fn an_older_server_is_unsupported_and_a_missing_one_not_found() {
    let older = Setup::installed_as(CODE, &[], &[("FAKE_CODE_FEATURES", "code-hint")]);

    let refused = index_server(&older).await;
    assert!(matches!(refused, Err(BusinessError::Unsupported(_))), "{refused:?}");

    let usage = get_code_index(&older.env, older.runner(), &older.binary).await;
    assert!(
        matches!(usage, Err(BusinessError::Unsupported(_))),
        "the usage error of an older server: {usage:?}"
    );

    let absent = Setup::without_server_as(CODE, &[], &[]);
    let refused = index_server(&absent).await;
    assert!(matches!(refused, Err(BusinessError::NotFound(_))), "{refused:?}");
}
