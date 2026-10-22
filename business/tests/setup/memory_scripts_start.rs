//! Starting the pooled `harness-memory mcp` server for scripts: without the binary nothing starts,
//! a server that exits is failed and probed again, and an upgraded binary is probed on a fresh
//! server (against the stand-in of `fake_agent`).

use harness_business::BusinessError;
use harness_business::ecosystem::{ListWindow, Repos, ScriptsQuery, ScriptsSupport};
use harness_business::model::MemoryScriptsAvailability;
use memory_stand_in::{CURRENT, LEGACY, Setup};
use pretty_assertions::assert_eq;

use crate::memory_stand_in;

fn query() -> ScriptsQuery {
    ScriptsQuery {
        window: ListWindow::new(None, None, None, None).unwrap(),
        group: None,
        scope: None,
        review: None,
        place: None,
        sort: None,
        hidden: false,
    }
}

#[tokio::test]
async fn without_the_binary_nothing_starts() {
    let setup = Setup::without_server(CURRENT, &[]);

    assert_eq!(
        setup
            .scripts
            .list(&setup.env, setup.runner(), Repos::Project(None), &query())
            .await
            .availability,
        MemoryScriptsAvailability::NotInstalled
    );

    let delete = setup
        .scripts
        .delete(&setup.env, setup.runner(), 7, Repos::Project(None))
        .await;
    assert!(matches!(delete, Err(BusinessError::NotFound(_))), "{delete:?}");
    assert_eq!(setup.scripts.support(&setup.env).await, ScriptsSupport::Unknown);
    assert!(setup.requested().is_empty());
}

#[tokio::test]
async fn a_server_that_does_not_start_is_failed_and_probed_again() {
    // `fake_agent` without a dialect exits at once with its usage.
    let setup = Setup::installed(&[], &[]);

    let MemoryScriptsAvailability::Failed { message } = setup.scripts.availability(&setup.env).await else {
        panic!("expected failed");
    };
    assert!(message.contains("exited before answering initialize"), "{message}");

    assert_eq!(
        setup.scripts.support(&setup.env).await,
        ScriptsSupport::Unknown,
        "a failed probe says nothing about scripts"
    );
    assert_eq!(setup.requested().len(), 2, "a failed probe is not cached");
}

#[tokio::test]
async fn a_binary_upgraded_in_place_is_probed_on_a_fresh_server() {
    let setup = Setup::installed(CURRENT, LEGACY);

    assert_eq!(
        setup.scripts.availability(&setup.env).await,
        MemoryScriptsAvailability::Outdated
    );

    // The old server is still pooled when the binary changes.
    setup.upgrade(&[]);

    assert_eq!(
        setup.scripts.availability(&setup.env).await,
        MemoryScriptsAvailability::Available
    );
    assert_eq!(setup.requested().len(), 2, "the new binary runs in a new server");

    setup.scripts.shutdown().await;
}
