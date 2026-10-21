//! One page of the scripts agents keep in Harness Memory (`memory_scripts_list`), from
//! `harness-memory scripts list --json` (see [`memory_cli`](super::memory_cli)), for
//! [`MemoryScripts::list`](super::memory_scripts::MemoryScripts::list).
//!
//! The list takes one `--group`, or `--scope repo|global` and `--review needed|trusted` combined
//! (never both kinds: the server refuses that as a usage error), and `--place local|team`. It
//! answers the counts of each group, with combined filters those of each scope (`scopes`, under
//! the query, review and place) and review (`review`, under the query, scope and place), and with
//! the place those of each place (`places`, `{"local", "team"}` under the query, scope and
//! review). A server that predates the place answers without `places` and refuses `--place`, one
//! that predates combined filters answers neither `scopes` nor `review` and refuses `--scope` and
//! `--review`: the list is then asked again without them (see
//! [`memory_list`](super::memory_list)).
//!
//! Each script says where it is kept (`place`), whether it is a read-only team script (`readOnly`,
//! scripts routed `remote+local`) and whether it is hidden on this machine (`hidden`); with a team
//! database the page also counts the hidden team scripts (`hiddenCount`) and the scripts of the
//! local file (`localCounts`, every repository). `--hidden` lists the team scripts hidden on this
//! machine instead, with the same filters and pages.

use std::ffi::OsString;
use std::path::Path;

use serde_json::Value;

use super::memory_cli::{ListWindow, Repos, count_of, counts_by_scope, list_availability, scope_counts, scope_name};
use super::memory_hidden::HIDDEN_FLAG;
use super::memory_list::{ListAnswer, list_filters, place_counts, run_list};
use super::memory_scripts::parse;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    MemoryScriptsAvailability, MemoryScriptsPage, ScriptGroup, ScriptGroupCounts, ScriptPlace, ScriptReview,
    ScriptReviewCounts, ScriptScope, ScriptSort, ScriptSummary,
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
    /// One place only, or both (dropped by a server that does not filter by place).
    pub place: Option<ScriptPlace>,
    /// `updated` unless given.
    pub sort: Option<ScriptSort>,
    /// List the team scripts hidden on this machine instead of the visible ones.
    pub hidden: bool,
}

impl ScriptsQuery {
    /// The query of the visible scripts of every place; `invalidInput` for a `group` with a
    /// `scope` or `review` (the server refuses that).
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
            place: None,
            sort,
            hidden: false,
        })
    }
}

/// One page of the scripts of `project` from the server at `server` (see
/// [`MemoryScripts::list`](super::memory_scripts::MemoryScripts::list)).
pub(super) async fn list_page(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: Repos<'_>,
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

    let mut combined = Vec::new();
    combined.extend(query.scope.map(|scope| ("--scope", scope_name(scope))));
    combined.extend(query.review.map(|review| ("--review", review_name(review))));

    let (filters, fallbacks) = list_filters(&group, &combined, query.place);

    let hidden = query.hidden.then(|| OsString::from(HIDDEN_FLAG));
    let args = |filters: &[(&str, &str)]| -> Vec<OsString> {
        [OsString::from("scripts")]
            .into_iter()
            .chain(query.window.list_args(filters, sort, project))
            .chain(hidden.clone())
            .collect()
    };
    let fallbacks = fallbacks.iter().map(|filters| args(filters)).collect();

    match run_list(env, runner, server, SCRIPTS_LIST_COMMAND, args(&filters), fallbacks).await {
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
        places: None,
        combined_filters: false,
        scripts: Vec::new(),
        hidden_count: None,
        local_counts: None,
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
        places: place_counts(data),
        combined_filters: answer.combined_filters,
        scripts,
        hidden_count: count_of(data.get("hiddenCount")),
        local_counts: counts_by_scope(data, "localCounts"),
    }
}
