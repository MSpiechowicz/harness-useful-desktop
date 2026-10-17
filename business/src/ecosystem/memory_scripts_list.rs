//! One page of the scripts agents keep in Harness Memory (`memory_scripts_list`), from
//! `harness-memory scripts list --json` (see [`memory_cli`](super::memory_cli)), for
//! [`MemoryScripts::list`](super::memory_scripts::MemoryScripts::list).
//!
//! The list takes one `--group`, or `--scope repo|global` and `--review needed|trusted` combined
//! (never both kinds: the server refuses that as a usage error), and answers the counts of each
//! group, and with combined filters those of each scope (`scopes`, under the query and review)
//! and review (`review`, under the query and scope). A server that predates combined filters
//! answers neither and refuses `--scope` and `--review`: the list is then asked once more without
//! them (see [`run_list`]).

use std::ffi::OsString;
use std::path::Path;

use serde_json::Value;

use super::memory_cli::{ListAnswer, ListWindow, count_of, list_availability, run_list, scope_counts, scope_name};
use super::memory_scripts::parse;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    MemoryScriptsAvailability, MemoryScriptsPage, ScriptGroup, ScriptGroupCounts, ScriptReview, ScriptReviewCounts,
    ScriptScope, ScriptSort, ScriptSummary,
};
use crate::process::ProcessRunner;

/// The command that lists scripts, in messages.
const SCRIPTS_LIST_COMMAND: &str = "scripts list";

/// Which scripts [`MemoryScripts::list`] reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptsQuery {
    /// The page and the search.
    pub window: ListWindow,
    /// One group only, or all. Never with `scope` or `review`.
    pub group: Option<ScriptGroup>,
    /// One scope only, or both (dropped by a server that does not combine filters).
    pub scope: Option<ScriptScope>,
    /// The scripts that need approval, the others, or all (dropped by a server that does not
    /// combine filters).
    pub review: Option<ScriptReview>,
    /// `updated` unless given.
    pub sort: Option<ScriptSort>,
}

impl ScriptsQuery {
    /// The query; `invalidInput` for a `group` with a `scope` or `review` (the server refuses
    /// that).
    pub fn new(
        window: ListWindow,
        group: Option<ScriptGroup>,
        scope: Option<ScriptScope>,
        review: Option<ScriptReview>,
        sort: Option<ScriptSort>,
    ) -> Result<Self> {
        if group.is_some() && (scope.is_some() || review.is_some()) {
            return Err(BusinessError::InvalidInput(
                "a script group does not combine with a scope or review filter".to_owned(),
            ));
        }

        Ok(Self {
            window,
            group,
            scope,
            review,
            sort,
        })
    }
}

/// One page of the scripts of `project` from the server at `server` (see
/// [`MemoryScripts::list`](super::memory_scripts::MemoryScripts::list)).
pub(super) async fn list_page(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: Option<&Path>,
    query: &ScriptsQuery,
) -> MemoryScriptsPage {
    let sort = match query.sort {
        Some(ScriptSort::Name) => "name",
        Some(ScriptSort::Language) => "language",
        Some(ScriptSort::Scope) => "scope",
        Some(ScriptSort::Place) => "place",
        Some(ScriptSort::Runs) => "runs",
        Some(ScriptSort::LastRun) => "lastRun",
        Some(ScriptSort::Updated) | None => "updated",
    };

    let group: Vec<(&str, &str)> = query
        .group
        .map(|group| ("--group", group_name(group)))
        .into_iter()
        .collect();

    let mut filters = group.clone();
    filters.extend(query.scope.map(|scope| ("--scope", scope_name(scope))));
    filters.extend(query.review.map(|review| ("--review", review_name(review))));

    let args = |filters: &[(&str, &str)]| -> Vec<OsString> {
        [OsString::from("scripts")]
            .into_iter()
            .chain(query.window.list_args(filters, sort, project))
            .collect()
    };
    let fallback = (filters.len() > group.len()).then(|| args(&group));

    match run_list(env, runner, server, SCRIPTS_LIST_COMMAND, args(&filters), fallback).await {
        Ok(answer) => scripts_page(answer),
        Err(error) => unavailable_page(list_availability(error)),
    }
}

/// The server's name of `group`.
fn group_name(group: ScriptGroup) -> &'static str {
    match group {
        ScriptGroup::NeedsReview => "needsReview",
        ScriptGroup::Repo => "repo",
        ScriptGroup::Global => "global",
    }
}

/// The server's name of `review`.
fn review_name(review: ScriptReview) -> &'static str {
    match review {
        ScriptReview::Needed => "needed",
        ScriptReview::Trusted => "trusted",
    }
}

/// An empty page that says why there are no scripts.
pub(super) fn unavailable_page(availability: MemoryScriptsAvailability) -> MemoryScriptsPage {
    MemoryScriptsPage {
        availability,
        total: 0,
        groups: ScriptGroupCounts::default(),
        scopes: None,
        review: None,
        combined_filters: false,
        scripts: Vec::new(),
    }
}

/// The page in a `scripts list --json` answer; an entry this app cannot read is skipped (logged).
/// A server that does not say `total` has the scripts it sent.
fn scripts_page(answer: ListAnswer) -> MemoryScriptsPage {
    let data = &answer.data;
    let entries = data
        .get("scripts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let scripts: Vec<ScriptSummary> = entries
        .into_iter()
        .filter_map(|entry| match parse::<ScriptSummary>(entry, SCRIPTS_LIST_COMMAND) {
            Ok(summary) => Some(summary),
            Err(_) => {
                tracing::warn!("memory scripts: an unreadable scripts list entry was skipped");
                None
            }
        })
        .collect();

    let groups = data.get("groups");
    let group = |name: &str| count_of(groups.and_then(|groups| groups.get(name))).unwrap_or(0);

    let review = data.get("review").filter(|review| review.is_object()).map(|review| {
        let count = |name: &str| count_of(review.get(name)).unwrap_or(0);

        ScriptReviewCounts {
            needed: count("needed"),
            trusted: count("trusted"),
        }
    });

    MemoryScriptsPage {
        availability: MemoryScriptsAvailability::Available,
        total: count_of(data.get("total")).unwrap_or(u32::try_from(scripts.len()).unwrap_or(u32::MAX)),
        groups: ScriptGroupCounts {
            needs_review: group("needsReview"),
            repo: group("repo"),
            global: group("global"),
        },
        scopes: scope_counts(data),
        review,
        combined_filters: answer.combined_filters,
        scripts,
    }
}
