//! Moving the knowledge database to the schema drafts need (`harness-knowledge upgrade --json`).
//!
//! The app runs it after it installed or updated Harness Knowledge ([`upgrade_knowledge`]) and when
//! `stats` answers that the database is not upgraded yet ([`read_stats_upgrading`]): a server with
//! `docs-drafts` opens every database it uses and upgrades it, so a teammate's older server never
//! reads drafts as live documents. A role that may not change the database cannot upgrade it: the
//! database then waits for a teammate's update, which [`KnowledgeUpgradeNotice`] remembers for the
//! pulse, with the one-time notice that running harnesses must be restarted.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::knowledge_cli::{call, run_json, words};
use super::knowledge_docs::{docs_server, read_stats};
use super::knowledge_features::KnowledgeFeatures;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{KnowledgeStats, KnowledgeUpgraded};
use crate::process::ProcessRunner;

/// How long the app waits before it tries to upgrade again after the database was still not
/// upgraded (a read-only role waits for a teammate, so asking every few seconds would be noise).
const RETRY_AFTER: Duration = Duration::from_secs(60);

/// What the server says while the database is not upgraded yet, as the words of its line.
const NOT_UPGRADED: &str = "not upgraded yet";

/// Whether `error` is the server saying the database is not upgraded yet.
pub fn is_not_upgraded(error: &BusinessError) -> bool {
    let message = match error {
        BusinessError::Process(message) | BusinessError::Protocol(message) => message,
        _ => return false,
    };

    message.to_ascii_lowercase().contains(NOT_UPGRADED)
}

/// Runs `upgrade --json` and answers what it did: `{"upgraded": bool, "waiting": bool}`, each
/// absent as false. `unsupported` for a server that predates it.
pub async fn upgrade_knowledge(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Result<KnowledgeUpgraded> {
    let data = run_json(env, runner, server, call("upgrade", words(&["upgrade", "--json"]))).await?;

    let flag = |name: &str| data.get(name).and_then(Value::as_bool).unwrap_or(false);

    tracing::info!(
        upgraded = flag("upgraded"),
        waiting = flag("waiting"),
        "knowledge upgrade ran"
    );

    Ok(KnowledgeUpgraded {
        upgraded: flag("upgraded"),
        waiting: flag("waiting"),
    })
}

/// What the app knows about the upgrade, for the pulse: whether running harnesses must be
/// restarted (until the user dismissed the notice), whether the database waits for a teammate, and
/// when the app last tried.
#[derive(Debug, Default)]
pub struct KnowledgeUpgradeNotice {
    restart: AtomicBool,
    waits: AtomicBool,
    tried: Mutex<Option<Instant>>,
}

impl KnowledgeUpgradeNotice {
    /// A database was upgraded and the user has not dismissed the notice.
    pub fn restart(&self) -> bool {
        self.restart.load(Ordering::Relaxed)
    }

    /// A database waits for an upgrade this role cannot make.
    pub fn waits(&self) -> bool {
        self.waits.load(Ordering::Relaxed)
    }

    /// Raises the restart notice without an upgrade: the installed server is the first with drafts on
    /// this computer, so a harness that started before holds an older one.
    pub fn raise_restart(&self) {
        self.restart.store(true, Ordering::Relaxed);
    }

    /// A read of `stats` succeeded: no database waits for an upgrade any more.
    pub fn clear_waits(&self) {
        self.waits.store(false, Ordering::Relaxed);
    }

    /// The user saw the restart notice.
    pub fn dismiss(&self) {
        self.restart.store(false, Ordering::Relaxed);
    }

    /// Remembers what an upgrade did (and that it just ran).
    pub fn record(&self, upgraded: KnowledgeUpgraded) {
        if upgraded.upgraded {
            self.restart.store(true, Ordering::Relaxed);
        }

        self.waits.store(upgraded.waiting, Ordering::Relaxed);
        *self.tried.lock().unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    }

    /// Whether an upgrade may run now: none ran yet, or the last one is [`RETRY_AFTER`] old. Takes the
    /// turn when it may.
    fn take_turn(&self) -> bool {
        let mut tried = self.tried.lock().unwrap_or_else(PoisonError::into_inner);

        if tried.is_some_and(|at| at.elapsed() < RETRY_AFTER) {
            return false;
        }

        *tried = Some(Instant::now());

        true
    }
}

/// `stats --json`, upgrading the database first when the server says it is not upgraded yet and
/// supports `upgrade` (`docs-drafts`): the answer after the upgrade, or the first answer when it
/// was not tried (too soon after the last try) or failed.
pub async fn read_stats_upgrading(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    server: &Path,
    notice: &KnowledgeUpgradeNotice,
) -> Result<KnowledgeStats> {
    let first = read_stats(env, runner, server).await;

    let Err(error) = &first else {
        return first;
    };

    if !is_not_upgraded(error) || !features.support(env, runner).await.docs_drafts() || !notice.take_turn() {
        return first;
    }

    match upgrade_knowledge(env, runner, server).await {
        Ok(upgraded) => notice.record(upgraded),
        Err(error) => {
            tracing::warn!(code = ?error.code(), "the knowledge database could not be upgraded");

            return first;
        }
    }

    read_stats(env, runner, server).await
}

/// Upgrades the database after Harness Knowledge was installed or updated, when the installed
/// server lists `docs-drafts`; `notice` remembers the result. With `first_drafts` (this computer
/// never had a server with drafts before) the restart notice is raised whether or not this
/// computer upgraded the database: a teammate may have done it, and a harness that started before
/// still runs the older server. A failure is logged and left for the next `stats` that says the
/// database is not upgraded yet. True when the installed server lists `docs-drafts`.
pub async fn upgrade_installed(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    notice: &KnowledgeUpgradeNotice,
    first_drafts: bool,
) -> bool {
    if !features.support(env, runner).await.docs_drafts() {
        return false;
    }

    if first_drafts {
        notice.raise_restart();
    }

    let Ok(server) = docs_server(env, runner, features).await else {
        return true;
    };

    match upgrade_knowledge(env, runner, &server).await {
        Ok(upgraded) => notice.record(upgraded),
        Err(error) => tracing::warn!(code = ?error.code(), "the knowledge database was not upgraded after the install"),
    }

    true
}
