//! The upgrade of the knowledge database that comes with drafts: run once after an install or when the counts say
//! the database is not upgraded yet, the one-time restart notice, and a role that waits for a teammate's upgrade.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::ecosystem::{
    KnowledgeFeatures, KnowledgeUpgradeNotice, knowledge_pulse, knowledge_pulse_upgrading, upgrade_installed,
};
use knowledge_stand_in::{ALL_FEATURES, BEFORE_DRAFTS, cli, drafts_stats, installed};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use team_support::{Cli, answers, exits, wait};

/// A server whose database is not upgraded until `upgrade` ran, which answers `reply`.
fn upgrading(reply: Value, upgraded: Arc<AtomicBool>) -> Cli {
    cli(ALL_FEATURES).answering(move |args| match args.first().map(String::as_str) {
        Some("upgrade") => {
            upgraded.store(reply["upgraded"].as_bool().unwrap_or(false), Ordering::SeqCst);

            Some(answers(reply.clone()))
        }
        Some("stats") if !upgraded.load(Ordering::SeqCst) => Some(exits(
            1,
            "",
            "harness-knowledge: the database is not upgraded yet, run `harness-knowledge upgrade`\n",
        )),
        Some("stats") => Some(answers(drafts_stats())),
        _ => None,
    })
}

fn upgrade_runs(cli: &Cli) -> usize {
    cli.commands().iter().filter(|command| command[0] == "upgrade").count()
}

#[test]
fn a_database_that_is_not_upgraded_is_upgraded_once_and_the_restart_notice_is_one_time() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let notice = KnowledgeUpgradeNotice::default();
    let server_cli = upgrading(json!({ "upgraded": true }), Arc::default());

    let pulse = wait(knowledge_pulse_upgrading(&env, &server_cli, &probe, &notice)).expect("answers after the upgrade");

    assert_eq!((pulse.documents, pulse.waiting), (12, Some(2)));
    assert_eq!((pulse.restart_harnesses, pulse.upgrade_waits), (Some(true), None));
    assert_eq!(upgrade_runs(&server_cli), 1);
    assert!(
        server_cli
            .commands()
            .contains(&vec!["upgrade".to_owned(), "--json".to_owned()])
    );

    let again = wait(knowledge_pulse_upgrading(&env, &server_cli, &probe, &notice)).expect("answers");
    assert_eq!(again.restart_harnesses, Some(true), "until the user dismissed it");

    notice.dismiss();
    let after = wait(knowledge_pulse_upgrading(&env, &server_cli, &probe, &notice)).expect("answers");
    assert_eq!(after.restart_harnesses, None);
    assert_eq!(upgrade_runs(&server_cli), 1, "nothing was upgraded again");
}

#[test]
fn a_role_that_cannot_upgrade_waits_for_a_teammate_and_is_not_asked_every_few_seconds() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let notice = KnowledgeUpgradeNotice::default();
    let server_cli = upgrading(json!({ "upgraded": false, "waiting": true }), Arc::default());

    let pulse = wait(knowledge_pulse_upgrading(&env, &server_cli, &probe, &notice)).expect("says it waits");
    assert_eq!(
        (pulse.documents, pulse.upgrade_waits, pulse.restart_harnesses),
        (0, Some(true), None)
    );

    let again = wait(knowledge_pulse_upgrading(&env, &server_cli, &probe, &notice)).expect("says it waits");
    assert_eq!(again.upgrade_waits, Some(true));
    assert_eq!(upgrade_runs(&server_cli), 1, "the next try is a minute away");

    // Without the notice (the plain counts) nothing is upgraded, and the wait is still said.
    let plain = upgrading(json!({ "upgraded": true }), Arc::default());
    let pulse = wait(knowledge_pulse(&env, &plain, &probe)).expect("says it waits");
    assert_eq!(pulse.upgrade_waits, Some(true));
    assert_eq!(upgrade_runs(&plain), 0);
}

#[test]
fn the_wait_for_a_teammates_upgrade_ends_when_the_counts_are_read_again() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let notice = KnowledgeUpgradeNotice::default();
    let upgraded = Arc::new(AtomicBool::new(false));
    let server_cli = upgrading(json!({ "upgraded": false, "waiting": true }), Arc::clone(&upgraded));

    let waiting = wait(knowledge_pulse_upgrading(&env, &server_cli, &probe, &notice)).expect("says it waits");
    assert_eq!(waiting.upgrade_waits, Some(true));
    assert!(notice.waits());

    // A teammate upgraded the database: the counts answer without this computer upgrading anything.
    upgraded.store(true, Ordering::SeqCst);
    let answered = wait(knowledge_pulse_upgrading(&env, &server_cli, &probe, &notice)).expect("answers");

    assert_eq!((answered.documents, answered.upgrade_waits), (12, None));
    assert!(!notice.waits());
    assert_eq!(upgrade_runs(&server_cli), 1, "this computer upgraded nothing");
}

#[test]
fn a_server_before_drafts_is_never_asked_to_upgrade() {
    let (_dir, env) = installed();
    let notice = KnowledgeUpgradeNotice::default();
    let old = cli(BEFORE_DRAFTS).starting(&["stats"], exits(1, "", "harness-knowledge: not upgraded yet\n"));

    assert!(!wait(upgrade_installed(
        &env,
        &old,
        &KnowledgeFeatures::new(),
        &notice,
        true
    )));
    let pulse = wait(knowledge_pulse_upgrading(
        &env,
        &old,
        &KnowledgeFeatures::new(),
        &notice,
    ))
    .expect("says it waits");

    assert_eq!(upgrade_runs(&old), 0);
    assert_eq!(pulse.upgrade_waits, Some(true));
    assert!(!notice.restart());
}

#[test]
fn after_an_install_the_database_is_upgraded_and_the_restart_notice_is_raised() {
    let (_dir, env) = installed();
    let notice = KnowledgeUpgradeNotice::default();
    let server_cli = cli(ALL_FEATURES).starting(&["upgrade"], answers(json!({ "upgraded": true })));

    assert!(wait(upgrade_installed(
        &env,
        &server_cli,
        &KnowledgeFeatures::new(),
        &notice,
        false
    )));

    assert!(notice.restart() && !notice.waits());
    assert_eq!(upgrade_runs(&server_cli), 1);

    // A failed upgrade is left for the next counts that say the database is not upgraded.
    let failing = cli(ALL_FEATURES).starting(&["upgrade"], exits(1, "", "harness-knowledge: locked\n"));
    let quiet = KnowledgeUpgradeNotice::default();
    wait(upgrade_installed(
        &env,
        &failing,
        &KnowledgeFeatures::new(),
        &quiet,
        false,
    ));
    assert!(!quiet.restart() && !quiet.waits());
}

#[test]
fn the_first_server_with_drafts_raises_the_restart_notice_even_when_no_upgrade_changed_anything() {
    let (_dir, env) = installed();

    // A teammate already upgraded the database: this computer's upgrade changes nothing.
    let teammate_did_it = cli(ALL_FEATURES).starting(&["upgrade"], answers(json!({ "upgraded": false })));
    let notice = KnowledgeUpgradeNotice::default();
    assert!(wait(upgrade_installed(
        &env,
        &teammate_did_it,
        &KnowledgeFeatures::new(),
        &notice,
        true
    )));
    assert!(notice.restart(), "a harness that started before holds the older server");
    assert!(!notice.waits());

    // Not the first time: nothing to restart for.
    let later = KnowledgeUpgradeNotice::default();
    assert!(wait(upgrade_installed(
        &env,
        &teammate_did_it,
        &KnowledgeFeatures::new(),
        &later,
        false
    )));
    assert!(!later.restart());

    // Even a failed upgrade does not hide it.
    let failing = cli(ALL_FEATURES).starting(&["upgrade"], exits(1, "", "harness-knowledge: locked\n"));
    let failed = KnowledgeUpgradeNotice::default();
    assert!(wait(upgrade_installed(
        &env,
        &failing,
        &KnowledgeFeatures::new(),
        &failed,
        true
    )));
    assert!(failed.restart());

    // A server before drafts has nothing to restart for, and says it is not the one.
    let old = KnowledgeUpgradeNotice::default();
    assert!(!wait(upgrade_installed(
        &env,
        &cli(BEFORE_DRAFTS),
        &KnowledgeFeatures::new(),
        &old,
        true
    )));
    assert!(!old.restart());
}
