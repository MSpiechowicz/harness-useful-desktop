//! `reconcile` against a switch turned off outside the app, a quitting app and a model removal.

use harness_business::ecosystem::{MemoryFeatures, Reconciled, memory_data_dir};
use harness_business::model::EmbedderPhase;
use pretty_assertions::assert_eq;

use super::{EMBEDDER, dead, embedder, only_embedder_runs, until};
use crate::memory_stand_in::Setup;

/// Writes `embedder.toml` where the server keeps it, as `embedder enable|disable` does.
fn write_switch_file(setup: &Setup, enabled: bool) {
    let folder = memory_data_dir(&setup.env).expect("a data folder");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("embedder.toml"), format!("enabled = {enabled}\n")).unwrap();
}

#[tokio::test]
async fn an_embedder_switched_off_on_the_command_line_stays_off_and_is_reported() {
    let chosen = embedder(&[("FAKE_EMBEDDER_ENABLED", "false")], &[10]);
    write_switch_file(&chosen.setup, false);

    let settled = chosen
        .supervisor
        .reconcile(
            &chosen.setup.env,
            chosen.setup.runner(),
            &MemoryFeatures::new(),
            true,
            &|| false,
        )
        .await;

    assert_eq!(settled, Reconciled::OptedOut);
    assert_eq!(chosen.starts(), 0, "nothing runs");
    assert_eq!(
        only_embedder_runs(&chosen.setup),
        [vec!["features"], vec!["embedder", "status", "--json"]],
        "it was not switched on"
    );
    assert_eq!(chosen.supervisor.state().phase, EmbedderPhase::Off);

    // Never chosen (no file): on by default, as before.
    let fresh = embedder(&[("FAKE_EMBEDDER_ENABLED", "false")], &[10]);
    let mut updates = fresh.supervisor.subscribe();

    let settled = fresh
        .supervisor
        .reconcile(
            &fresh.setup.env,
            fresh.setup.runner(),
            &MemoryFeatures::new(),
            true,
            &|| false,
        )
        .await;
    until(&mut updates, |state| state.phase == EmbedderPhase::Ready).await;

    assert_eq!(settled, Reconciled::Settled);
    assert!(only_embedder_runs(&fresh.setup).contains(&vec!["embedder".to_owned(), "enable".to_owned()]));
    fresh.supervisor.stop().await;
}

#[tokio::test]
async fn an_embedder_that_runs_when_it_is_found_switched_off_is_stopped() {
    let running = embedder(&[("FAKE_EMBEDDER_ENABLED", "true")], &[10]);
    let mut updates = running.supervisor.subscribe();
    running.supervisor.start(running.spec()).await;
    until(&mut updates, |state| state.phase == EmbedderPhase::Ready).await;

    write_switch_file(&running.setup, false);
    std::fs::write(running.setup.env.home.join(".fake-embedder-switch"), "disable").unwrap();

    let settled = running
        .supervisor
        .reconcile(
            &running.setup.env,
            running.setup.runner(),
            &MemoryFeatures::new(),
            true,
            &|| false,
        )
        .await;

    assert_eq!(settled, Reconciled::OptedOut);
    assert_eq!(running.supervisor.state().phase, EmbedderPhase::Off);
    assert!(dead(running.pids()[0]).await);
}

#[tokio::test]
async fn nothing_starts_once_the_app_is_quitting() {
    let quitting = embedder(&[("FAKE_EMBEDDER_ENABLED", "true")], &[10]);

    quitting
        .supervisor
        .reconcile(
            &quitting.setup.env,
            quitting.setup.runner(),
            &MemoryFeatures::new(),
            true,
            &|| true,
        )
        .await;

    assert_eq!(quitting.starts(), 0);
    assert_eq!(quitting.supervisor.state().phase, EmbedderPhase::Off);

    assert!(!quitting.supervisor.start_unless(quitting.spec(), &|| true).await);
    assert_eq!(quitting.starts(), 0);
}

#[tokio::test]
async fn a_reconcile_cannot_restart_the_embedder_between_its_stop_and_the_removal_of_the_model() {
    let removing = embedder(
        &[
            ("FAKE_EMBEDDER_ENABLED", "true"),
            // It ignores its stdin, so stopping it takes the whole grace: the time a reconcile would use.
            (
                "FAKE_EMBEDDER_SCRIPT",
                "emit {\"event\":\"ready\",\"model\":\"m1\",\"dim\":384}\nhang",
            ),
        ],
        &[10],
    );
    let mut updates = removing.supervisor.subscribe();
    removing.supervisor.start(removing.spec()).await;
    until(&mut updates, |state| state.phase == EmbedderPhase::Ready).await;

    let features = MemoryFeatures::new();
    let (removed, _) = tokio::join!(
        removing
            .supervisor
            .remove_model(&removing.setup.env, removing.setup.runner(), &removing.setup.binary),
        removing
            .supervisor
            .reconcile(&removing.setup.env, removing.setup.runner(), &features, true, &|| false,),
    );
    removed.expect("removed");

    let requested = removing.setup.requested();
    let removal = requested
        .iter()
        .position(|spec| spec.args.last().is_some_and(|arg| arg == "remove-model"))
        .expect("the model was removed");
    let starts: Vec<usize> = requested
        .iter()
        .enumerate()
        .filter(|(_, spec)| spec.args == EMBEDDER)
        .map(|(index, _)| index)
        .collect();

    assert!(
        starts.iter().skip(1).all(|start| *start > removal),
        "an embedder started before the removal: {starts:?} / {removal}"
    );

    removing.supervisor.stop().await;
}
