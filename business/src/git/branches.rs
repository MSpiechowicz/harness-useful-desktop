//! `git_branches` from one `git for-each-ref` over `refs/heads` and `refs/remotes`.
//!
//! Branches come newest first by committer date, local ones before remote ones, at most
//! [`MAX_BRANCHES`]. Remote `HEAD` aliases are skipped. Ahead/behind come from
//! `%(upstream:track)`; a gone upstream reports neither.

use std::time::{Duration, UNIX_EPOCH};

use super::cli::GitCli;
use super::status::SHORT_SHA;
use crate::agent::rfc3339;
use crate::error::Result;
use crate::model::{Branch, CommitInfo, MAX_BRANCHES};

/// Field separator (`%00`) and record terminator (`%1e`) of [`FORMAT`].
const FIELD: char = '\0';
const RECORD: char = '\u{1e}';

/// Fields: ref, HEAD marker, upstream, track, sha, subject, author, committer date (unix).
const FORMAT: &str = concat!(
    "--format=%(refname)%00%(HEAD)%00%(upstream:short)%00%(upstream:track,nobracket)%00",
    "%(objectname:short=7)%00%(contents:subject)%00%(authorname)%00%(committerdate:unix)%1e"
);

/// The project's local and remote branches; empty outside a work tree.
pub async fn git_branches(git: &GitCli<'_>) -> Result<Vec<Branch>> {
    if git.prefix().await?.is_none() {
        return Ok(Vec::new());
    }

    let output = git
        .run_ok([
            "for-each-ref",
            "--sort=-committerdate",
            FORMAT,
            "refs/heads/",
            "refs/remotes/",
        ])
        .await?;

    Ok(parse_branches(&output.stdout))
}

/// Parses [`FORMAT`] records (see the module docs for order and limits).
pub fn parse_branches(raw: &str) -> Vec<Branch> {
    let (mut local, mut remote): (Vec<Branch>, Vec<Branch>) = raw
        .split(RECORD)
        .filter_map(|record| parse_record(record.trim_start_matches('\n')))
        .partition(|branch| !branch.is_remote);

    local.append(&mut remote);
    local.truncate(MAX_BRANCHES);
    local
}

fn parse_record(record: &str) -> Option<Branch> {
    let fields: Vec<&str> = record.split(FIELD).collect();
    let [refname, head, upstream, track, sha, subject, author, date] = fields.as_slice() else {
        return None;
    };

    let (name, is_remote) = match (
        refname.strip_prefix("refs/heads/"),
        refname.strip_prefix("refs/remotes/"),
    ) {
        (Some(name), _) => (name, false),
        (None, Some(name)) if !name.ends_with("/HEAD") => (name, true),
        _ => return None,
    };

    let upstream = (!upstream.is_empty()).then(|| (*upstream).to_owned());
    let (ahead, behind) = match &upstream {
        Some(_) => parse_track(track),
        None => (None, None),
    };

    let seconds = date.parse::<u64>().ok();
    let last_commit = (!sha.is_empty()).then(|| CommitInfo {
        sha: sha.chars().take(SHORT_SHA).collect(),
        summary: (*subject).to_owned(),
        author: (*author).to_owned(),
        date: seconds.map_or_else(String::new, |seconds| {
            rfc3339(UNIX_EPOCH + Duration::from_secs(seconds))
        }),
    });

    Some(Branch {
        name: name.to_owned(),
        is_current: *head == "*",
        is_remote,
        upstream,
        ahead,
        behind,
        last_commit,
    })
}

/// `ahead 2, behind 1` / `ahead 2` / `behind 1` / `` (in sync) / `gone`.
fn parse_track(track: &str) -> (Option<u32>, Option<u32>) {
    if track == "gone" {
        return (None, None);
    }

    let mut counts = (Some(0), Some(0));

    for part in track.split(", ") {
        if let Some(ahead) = part.strip_prefix("ahead ") {
            counts.0 = ahead.parse().ok();
        } else if let Some(behind) = part.strip_prefix("behind ") {
            counts.1 = behind.parse().ok();
        }
    }

    counts
}
