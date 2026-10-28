//! The list commands of `harness-memory` (`notes list`, `scripts list`) asked with the filters the
//! installed server knows, for [`memory_notes`](super::memory_notes) and
//! [`memory_scripts_list`](super::memory_scripts_list).
//!
//! The filters came in generations: those of every server with pages (`--kind`, `--group`), the
//! combined ones (`--scope`, `--review`), then the place (`--place local|team`). A server refuses
//! an option it predates as a usage error, so a list is asked again without the place, then
//! without the combined filters too ([`list_filters`], [`run_list`]). A server that combines
//! filters answers the counts of each scope (`scopes`), one that knows the place those of each
//! place (`places`, `{"local": N, "team": M}` under every other filter): a page without `places`
//! keeps every place.

use std::ffi::OsString;
use std::path::Path;

use serde_json::Value;

use super::memory_cli::{CliCall, LIST_OUTPUT_BYTES, LIST_TIMEOUT, count_of, run_json};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{MemoryPlaceCounts, ScriptPlace};
use crate::process::ProcessRunner;

/// One option of a list command and its value (`("--kind", "fix")`).
pub(crate) type Filter<'a> = (&'a str, &'a str);

/// The filters a list asks with, and those it asks again with, in turn, while the server refuses
/// them as a usage error: without `--place`, then without `combined` (see the module docs). Only
/// the steps that drop a filter asked for are kept.
pub(crate) fn list_filters<'a>(
    base: &[Filter<'a>],
    combined: &[Filter<'a>],
    place: Option<ScriptPlace>,
) -> (Vec<Filter<'a>>, Vec<Vec<Filter<'a>>>) {
    let with_combined: Vec<Filter<'a>> = base.iter().chain(combined).copied().collect();
    let mut fallbacks = Vec::new();

    let filters = match place {
        Some(place) => {
            let mut filters = with_combined.clone();
            filters.push(("--place", place_name(place)));
            fallbacks.push(with_combined);
            filters
        }
        None => with_combined,
    };

    if !combined.is_empty() {
        fallbacks.push(base.to_vec());
    }

    (filters, fallbacks)
}

/// The server's name of `place`.
fn place_name(place: ScriptPlace) -> &'static str {
    match place {
        ScriptPlace::Local => "local",
        ScriptPlace::Team => "team",
    }
}

/// A list command's JSON answer, and whether the server combines filters.
pub(crate) struct ListAnswer {
    /// The answer.
    pub data: Value,
    /// True when the server answered the counts of each scope, as one that combines filters does
    /// (it took `--scope` and `--review`).
    pub combined_filters: bool,
}

/// Runs list command `name` with `args`, and while the server refuses them as a usage error (an
/// older server) with each of `fallbacks` in turn (see [`list_filters`]). Fails as [`run_json`]:
/// a server that refuses every one is `unsupported`.
pub(crate) async fn run_list(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    name: &str,
    args: Vec<OsString>,
    fallbacks: Vec<Vec<OsString>>,
) -> Result<ListAnswer> {
    let call = |args| CliCall {
        name,
        args,
        timeout: LIST_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin: None,
    };

    let mut answer = run_json(env, runner, server, call(args)).await;

    for fallback in fallbacks {
        if !matches!(answer, Err(BusinessError::Unsupported(_))) {
            break;
        }

        tracing::info!(
            command = name,
            "harness-memory predates a list filter; listing without it"
        );

        answer = run_json(env, runner, server, call(fallback)).await;
    }

    let data = answer?;
    let combined_filters = data.get("scopes").is_some_and(Value::is_object);

    Ok(ListAnswer { data, combined_filters })
}

/// Whether a list answer came from this computer's copy because the team database was unreachable
/// (`"offline": true`, a server with the `offline-fallback` feature): the team's entries are
/// missing from it, so it must not stand for what is kept.
pub(crate) fn offline(data: &Value) -> bool {
    data.get("offline").and_then(Value::as_bool).unwrap_or(false)
}

/// The `places` counts of a list answer; `None` when the server does not send them.
pub(crate) fn place_counts(data: &Value) -> Option<MemoryPlaceCounts> {
    let places = data.get("places").filter(|places| places.is_object())?;

    Some(MemoryPlaceCounts {
        local: count_of(places.get("local")).unwrap_or(0),
        team: count_of(places.get("team")).unwrap_or(0),
    })
}
