//! The main branch and the remote of the sync state (see [`super::repo_state`]).
//!
//! The main branch is, in this order: the one the user picked (ignored when it no longer exists),
//! the default branch of the `upstream` remote, the default branch of `origin`, `init.defaultBranch`
//! when such a local branch exists, and the first of `main`, `master` and `trunk`. Its reference is
//! the remote-tracking branch when there is one, so an unpulled main still counts.

use std::collections::HashMap;

use super::branch_ops::{validate_branch_name, validate_remote_name};
use super::cli::GitCli;
use crate::error::Result;
use crate::model::{MainBranch, MainSource};

/// Branch names tried, in this order, when git names no default branch.
const FALLBACK_MAINS: [&str; 3] = ["main", "master", "trunk"];

/// The configuration values the sync state needs.
#[derive(Default)]
pub(super) struct Config {
    init_default_branch: Option<String>,
    push_default: Option<String>,
    /// `branch.<name>.remote` per branch.
    branch_remotes: HashMap<String, String>,
}

pub(super) async fn read_config(git: &GitCli<'_>) -> Result<Config> {
    let output = git
        .run([
            "config",
            "--get-regexp",
            r"^(init\.defaultbranch|remote\.pushdefault|branch\..*\.remote)$",
        ])
        .await?;
    let mut config = Config::default();

    // Exit code 1 means no such key is set.
    for line in output.stdout.lines() {
        let Some((key, value)) = line.split_once(' ') else {
            continue;
        };

        match key {
            "init.defaultbranch" => config.init_default_branch = Some(value.to_owned()),
            "remote.pushdefault" => config.push_default = Some(value.to_owned()),
            _ => {
                let branch = key
                    .strip_prefix("branch.")
                    .and_then(|rest| rest.strip_suffix(".remote"));

                if let Some(branch) = branch {
                    config.branch_remotes.insert(branch.to_owned(), value.to_owned());
                }
            }
        }
    }

    Ok(config)
}

/// The upstream's remote, else `remote.pushDefault`, else `origin`, else the only remote.
pub(super) fn choose_remote(remotes: &[String], config: &Config, branch: Option<&str>) -> Option<String> {
    let known = |name: &&String| remotes.contains(*name);

    let upstream = branch.and_then(|branch| config.branch_remotes.get(branch));

    if let Some(remote) = upstream.filter(known) {
        return Some(remote.clone());
    }

    if let Some(remote) = config.push_default.as_ref().filter(known) {
        return Some(remote.clone());
    }

    if remotes.iter().any(|name| name == "origin") {
        return Some("origin".to_owned());
    }

    match remotes {
        [only] => Some(only.clone()),
        _ => None,
    }
}

/// A main branch found among the refs, before its new commits are counted.
pub(super) struct FoundMain {
    pub(super) name: String,
    reference: String,
    source: MainSource,
    pub(super) oid: String,
}

/// One ref as `for-each-ref` lists it.
struct RefInfo {
    oid: String,
    /// The ref a symbolic ref points at.
    symref: Option<String>,
}

pub(super) async fn find_main(
    git: &GitCli<'_>,
    remotes: &[String],
    sync_remote: Option<&str>,
    config: &Config,
    picked: Option<&str>,
) -> Result<Option<FoundMain>> {
    // Remotes whose branches can be the main one: the sync remote first, then the usual two.
    let mut relevant: Vec<&str> = Vec::new();

    for name in [sync_remote, Some("upstream"), Some("origin")].into_iter().flatten() {
        let usable = remotes.iter().any(|remote| remote == name) && validate_remote_name(name).is_ok();

        if usable && !relevant.contains(&name) {
            relevant.push(name);
        }
    }

    let default_branch = config.init_default_branch.as_deref();
    let names: Vec<&str> = [picked, default_branch]
        .into_iter()
        .flatten()
        .filter(|name| validate_branch_name(name).is_ok())
        .chain(FALLBACK_MAINS)
        .collect();

    let mut patterns = Vec::new();

    for name in &names {
        patterns.push(format!("refs/heads/{name}"));
        patterns.extend(relevant.iter().map(|remote| format!("refs/remotes/{remote}/{name}")));
    }

    patterns.extend(relevant.iter().map(|remote| format!("refs/remotes/{remote}/HEAD")));

    let refs = read_refs(git, &patterns).await?;
    let resolve = |name: &str, source: MainSource, remote_only_ok: bool| {
        let local = refs.get(&format!("refs/heads/{name}"));

        if !remote_only_ok && local.is_none() {
            return None;
        }

        let tracking = relevant.iter().find_map(|remote| {
            refs.get(&format!("refs/remotes/{remote}/{name}"))
                .map(|info| (format!("{remote}/{name}"), info))
        });

        let (reference, info) = match (tracking, local) {
            (Some(found), _) => found,
            (None, Some(info)) => (name.to_owned(), info),
            (None, None) => return None,
        };

        Some(FoundMain {
            name: name.to_owned(),
            reference,
            source,
            oid: info.oid.clone(),
        })
    };

    if let Some(found) = picked.and_then(|name| resolve(name, MainSource::Picked, true)) {
        return Ok(Some(found));
    }

    for remote in ["upstream", "origin"] {
        let Some(info) = refs.get(&format!("refs/remotes/{remote}/HEAD")) else {
            continue;
        };
        let Some(reference) = info
            .symref
            .as_deref()
            .and_then(|target| target.strip_prefix("refs/remotes/"))
        else {
            continue;
        };

        // The symref is a file of the repository, so its name is checked like any other.
        if let Some(name) = reference.strip_prefix(&format!("{remote}/"))
            && validate_branch_name(name).is_ok()
        {
            return Ok(Some(FoundMain {
                name: name.to_owned(),
                reference: reference.to_owned(),
                source: MainSource::Remote,
                oid: info.oid.clone(),
            }));
        }
    }

    if let Some(found) = default_branch.and_then(|name| resolve(name, MainSource::Fallback, false)) {
        return Ok(Some(found));
    }

    Ok(FALLBACK_MAINS
        .iter()
        .find_map(|name| resolve(name, MainSource::Fallback, true)))
}

/// The refs matching `patterns` (full ref names, or a remote's `HEAD`).
async fn read_refs(git: &GitCli<'_>, patterns: &[String]) -> Result<HashMap<String, RefInfo>> {
    let mut args = vec![
        "for-each-ref".to_owned(),
        "--format=%(refname)%00%(objectname)%00%(symref)".to_owned(),
    ];
    args.extend(patterns.iter().cloned());

    let output = git.run_ok(args).await?;
    let mut refs = HashMap::new();

    for line in output.stdout.lines() {
        let fields: Vec<&str> = line.split('\0').collect();
        let [name, oid, symref] = fields.as_slice() else {
            continue;
        };

        refs.insert(
            (*name).to_owned(),
            RefInfo {
                oid: (*oid).to_owned(),
                symref: (!symref.is_empty()).then(|| (*symref).to_owned()),
            },
        );
    }

    Ok(refs)
}

/// `found` with the commits it is ahead of HEAD counted.
pub(super) async fn main_branch(git: &GitCli<'_>, found: FoundMain, current: Option<&str>) -> Result<MainBranch> {
    let range = format!("HEAD..{}", found.reference);
    let output = git.run(["rev-list", "--count", "--end-of-options", &range]).await?;
    let new_commits = if output.success() {
        output.stdout.trim().parse().unwrap_or(0)
    } else {
        0
    };

    Ok(MainBranch {
        is_current: current == Some(found.name.as_str()),
        name: found.name,
        reference: found.reference,
        source: found.source,
        new_commits,
    })
}
