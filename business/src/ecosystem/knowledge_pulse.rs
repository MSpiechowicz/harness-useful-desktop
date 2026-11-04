//! The counts the live view follows ([`knowledge_pulse`], [`knowledge_pulse_upgrading`]), all from one
//! `harness-knowledge stats --json` call: the documents, the generation, how many proposals wait,
//! how many documents wait for this person's approval (and in all, `waiting_all`) and whether team approval is on.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use super::knowledge_cli::{call, run_json, words};
use super::knowledge_docs::{docs_server, read_stats};
use super::knowledge_features::KnowledgeFeatures;
use super::knowledge_held::knowledge_held_server;
use super::knowledge_upgrade::{KnowledgeUpgradeNotice, is_not_upgraded, read_stats_upgrading};
use crate::env::Env;
use crate::error::BusinessError;
use crate::model::{KnowledgePlaceStats, KnowledgePulse, KnowledgeStats, MAX_AUTHOR_NAME_CHARS, display_text};
use crate::process::ProcessRunner;

/// The bits of the combined generation that are kept: a JavaScript number holds 53 exactly.
const GENERATION_MASK: u64 = (1 << 52) - 1;

/// The counts the live view follows, all from one `stats --json` call: the documents, the
/// generation, how many proposals wait (`held`), how many documents wait for this person's approval
/// (`waiting`, and `waiting_all` in every place) and whether team approval is on. A server that predates `held` in its counts is asked for `held
/// list --json` instead, while it has a review queue. `None` while the server cannot answer
/// `stats`. A database that is not upgraded yet is not upgraded here: see
/// [`knowledge_pulse_upgrading`].
pub async fn knowledge_pulse(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
) -> Option<KnowledgePulse> {
    pulse_of(env, runner, features, None).await
}

/// [`knowledge_pulse`] that runs `upgrade --json` when the counts say the database is not upgraded
/// yet (`notice` remembers what it did, and when it tried last), and carries the notice's flags.
pub async fn knowledge_pulse_upgrading(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    notice: &KnowledgeUpgradeNotice,
) -> Option<KnowledgePulse> {
    let mut pulse = pulse_of(env, runner, features, Some(notice)).await?;

    pulse.restart_harnesses = notice.restart().then_some(true);
    pulse.upgrade_waits = pulse.upgrade_waits.or_else(|| notice.waits().then_some(true));

    Some(pulse)
}

async fn pulse_of(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    notice: Option<&KnowledgeUpgradeNotice>,
) -> Option<KnowledgePulse> {
    let server = docs_server(env, runner, features).await.ok()?;

    let read = match notice {
        Some(notice) => read_stats_upgrading(env, runner, features, &server, notice).await,
        None => read_stats(env, runner, &server).await,
    };

    let stats = match read {
        Ok(stats) => {
            // The database answers: whatever waited for an upgrade has been upgraded (by a teammate).
            if let Some(notice) = notice {
                notice.clear_waits();
            }

            stats
        }
        // The remote is the only place in use and cannot be reached (with both, the server falls
        // back to this computer and answers): nothing is known, and it is said so, so the last
        // counts are not taken for current ones.
        Err(BusinessError::Unreachable(_)) => {
            return Some(KnowledgePulse {
                offline: true,
                ..empty_pulse()
            });
        }
        // The database waits for an upgrade this role cannot make.
        Err(error) if is_not_upgraded(&error) => {
            return Some(KnowledgePulse {
                upgrade_waits: Some(true),
                ..empty_pulse()
            });
        }
        Err(_) => return None,
    };

    let held = match stats.held {
        Some(held) => Some(held),
        None => match knowledge_held_server(env, runner, features).await {
            Ok(server) => held_total(env, runner, &server).await,
            Err(_) => None,
        },
    };

    let (documents, generation) = combined_counts(&stats);

    Some(KnowledgePulse {
        documents,
        generation,
        held,
        offline: stats.offline,
        waiting: waiting_for_you(&stats),
        waiting_all: waiting_total(&stats),
        approval: stats.approval,
        me: viewer_name(&stats),
        ..empty_pulse()
    })
}

/// A pulse without counts: the base of the ones that know none.
fn empty_pulse() -> KnowledgePulse {
    KnowledgePulse {
        documents: 0,
        generation: 0,
        held: None,
        offline: false,
        waiting: None,
        waiting_all: None,
        approval: None,
        restart_harnesses: None,
        upgrade_waits: None,
        me: None,
    }
}

/// The name the server says the person has (`me.name`), trimmed, none when blank or not said.
fn viewer_name(stats: &KnowledgeStats) -> Option<String> {
    let name = display_text(stats.me.as_ref()?.name.as_deref()?, MAX_AUTHOR_NAME_CHARS);

    (!name.is_empty()).then_some(name)
}

/// How many documents wait for this person's approval: the server's `waitingForYou`. A server that
/// counts waiting documents but predates it (no `waitingForYou`) gives `0`, never the total, so
/// nobody is badged for their own documents. `None` from a server that counts no waiting
/// documents.
fn waiting_for_you(stats: &KnowledgeStats) -> Option<u32> {
    let counts_waiting = waiting_total(stats).is_some();

    counts_waiting.then(|| stats.waiting_for_you.unwrap_or(0))
}

/// How many documents wait for approval in every place: the server's top-level count, else the
/// sum of the places that say. `None` from a server that does not count them.
fn waiting_total(stats: &KnowledgeStats) -> Option<u32> {
    if stats.waiting.is_some() {
        return stats.waiting;
    }

    let places = stats.places.as_ref()?;
    let counts: Vec<u32> = [&places.remote, &places.local]
        .into_iter()
        .flatten()
        .filter_map(|place| place.waiting)
        .collect();

    (!counts.is_empty()).then(|| counts.iter().fold(0_u32, |sum, count| sum.saturating_add(*count)))
}

/// The documents and the generation the view follows. With both places in use the top-level
/// numbers are the team database's alone, so they are the places' that can be reached: the sum of
/// their documents, and one number that moves when any of them changes (a local import must
/// reload the list too). Otherwise the top-level numbers.
fn combined_counts(stats: &KnowledgeStats) -> (u32, i64) {
    let Some(places) = &stats.places else {
        return (stats.documents, stats.generation);
    };

    let reachable: Vec<&KnowledgePlaceStats> = [&places.remote, &places.local]
        .into_iter()
        .flatten()
        .filter(|place| !place.offline)
        .collect();

    let documents = reachable
        .iter()
        .fold(0_u32, |sum, place| sum.saturating_add(place.documents));

    let mut hasher = DefaultHasher::new();

    for place in &reachable {
        (
            place.generation,
            place.documents,
            place.chunks,
            place.embedded_chunks,
            place.drafts,
            place.waiting,
            place.pending,
        )
            .hash(&mut hasher);
    }

    // 52 bits, so the number stays exact as a JavaScript number and positive.
    let generation = i64::try_from(hasher.finish() & GENERATION_MASK).unwrap_or_default();

    (documents, generation)
}

/// How many proposals wait: the length of `held list`'s `held`.
async fn held_total(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Option<u32> {
    let data = run_json(
        env,
        runner,
        server,
        call("held list", words(&["held", "list", "--json"])),
    )
    .await
    .ok()?;

    let held = data.get("held")?.as_array()?;

    Some(u32::try_from(held.len()).unwrap_or(u32::MAX))
}
