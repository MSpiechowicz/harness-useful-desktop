//! The documents of Harness Knowledge, through `harness-knowledge docs list|get|links|rename|delete`
//! (15 s each), `repos --json` and `stats --json` (`knowledge_docs_list`, `knowledge_doc_get`,
//! `knowledge_doc_links_set`, `knowledge_doc_rename`, `knowledge_docs_delete`,
//! `knowledge_repos_list`, `knowledge_stats`). The app never opens the database.
//!
//! Every command needs `docs` in the server's features: `notFound` when Harness Knowledge is not
//! installed, `unsupported` when it predates them. Lists never fail for it: what went wrong is the
//! page's availability. The server counts pages from 1, so the page of a [`ListWindow`] must start
//! at a multiple of its size. The server's filters are exact, so what the app sends is checked
//! first (`invalidInput`): a format name, a repository key, a title, the links.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::knowledge_cli::{
    CliFailure, availability_of, availability_of_failure, call, count_of, field, run_json, run_typed, words,
};
use super::knowledge_features::{KnowledgeFeatures, KnowledgeSupport};
use super::sidecar_cli::{CliCall, LIST_OUTPUT_BYTES, LIST_TIMEOUT, ListWindow, is_repo_key};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    KnowledgeAvailability, KnowledgeChunk, KnowledgeDeleted, KnowledgeDetail, KnowledgeDocument, KnowledgeLink,
    KnowledgeListFilter, KnowledgePage, KnowledgeRepoCount, KnowledgeReposPage, KnowledgeScope, KnowledgeSort,
    KnowledgeStats,
};
use crate::process::ProcessRunner;

/// The longest search the server takes, in characters (a window allows more).
const MAX_QUERY_CHARS: usize = 200;

/// The longest title, in characters.
const MAX_TITLE_CHARS: usize = 300;

/// The most links of a document, and the most globs of a link.
const MAX_LINKS: usize = 32;

/// The longest path glob, in characters.
const MAX_GLOB_CHARS: usize = 256;

/// The most ids one delete takes.
const MAX_DELETE_IDS: usize = 1000;

/// The sorts that go largest or newest first unless asked otherwise.
const DESCENDING_SORTS: [KnowledgeSort; 2] = [KnowledgeSort::Updated, KnowledgeSort::Size];

/// The installed server when it lists `docs`.
pub async fn docs_server(env: &Env, runner: &dyn ProcessRunner, features: &KnowledgeFeatures) -> Result<PathBuf> {
    features
        .feature_server(env, runner, KnowledgeSupport::docs, "documents")
        .await
}

/// One page of documents. `invalidInput` for a filter or a page the server would refuse (see the
/// module docs); what is wrong with the server is the page's availability (see the module docs).
pub async fn list_docs(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    filter: &KnowledgeListFilter,
    window: &ListWindow,
) -> Result<KnowledgePage> {
    let args = list_args(filter, window)?;

    let server = match docs_server(env, runner, features).await {
        Ok(server) => server,
        Err(error) => return Ok(KnowledgePage::unavailable(availability_of(error))),
    };

    let page = match run_typed(env, runner, &server, call("docs list", args)).await {
        Ok(data) => read_page(&data).map_err(CliFailure::from),
        Err(failure) => Err(failure),
    };

    Ok(page.unwrap_or_else(|failure| KnowledgePage::unavailable(availability_of_failure(failure))))
}

/// The page in the answer of `docs list`.
fn read_page(data: &Value) -> Result<KnowledgePage> {
    Ok(KnowledgePage {
        availability: KnowledgeAvailability::Available,
        items: field::<Vec<KnowledgeDocument>>(data, "items", "docs list")?,
        total: count_of(data.get("total")),
    })
}

/// `docs list --json [--repo KEY | --global] [--format F] [--query Q] --sort S [--desc] --page N
/// --per-page N`.
pub fn list_args(filter: &KnowledgeListFilter, window: &ListWindow) -> Result<Vec<std::ffi::OsString>> {
    let mut args = words(&["docs", "list", "--json"]);

    match &filter.scope {
        Some(KnowledgeScope::Global) => args.push("--global".into()),
        Some(KnowledgeScope::Repo { key }) => {
            check_repo_key(key)?;
            args.extend(["--repo".into(), key.into()]);
        }
        Some(KnowledgeScope::All) | None => {}
    }

    if let Some(format) = &filter.format {
        check_format(format)?;
        args.extend(["--format".into(), format.into()]);
    }

    if let Some(query) = window.query() {
        if query.chars().count() > MAX_QUERY_CHARS {
            return Err(invalid(&format!("a search is at most {MAX_QUERY_CHARS} characters")));
        }

        args.extend(["--query".into(), query.into()]);
    }

    let sort = filter.sort.unwrap_or(KnowledgeSort::Updated);
    args.extend(["--sort".into(), sort_name(sort).into()]);

    if window.descending(DESCENDING_SORTS.contains(&sort)) {
        args.push("--desc".into());
    }

    let (page, per_page) = window.numbered()?;
    args.extend(["--page".into(), page.to_string().into()]);
    args.extend(["--per-page".into(), per_page.to_string().into()]);

    Ok(args)
}

/// The server's name of `sort`.
fn sort_name(sort: KnowledgeSort) -> &'static str {
    match sort {
        KnowledgeSort::Title => "title",
        KnowledgeSort::Updated => "updated",
        KnowledgeSort::Size => "size",
    }
}

/// A document with its outline, and the chunk `chunk` of it with its text when asked (`docs get
/// --json ID --sections [--chunk CID]`). `process` with the server's line for an id that does not
/// exist.
pub async fn get_doc(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: i64,
    chunk: Option<i64>,
) -> Result<KnowledgeDetail> {
    let mut args = words(&["docs", "get", "--json"]);
    args.extend([check_id(id)?.to_string().into(), "--sections".into()]);

    if let Some(chunk) = chunk {
        args.extend(["--chunk".into(), check_id(chunk)?.to_string().into()]);
    }

    let data = run_json(env, runner, server, call("docs get", args)).await?;

    let chunk = if data.get("chunk").is_some() {
        Some(field::<KnowledgeChunk>(&data, "chunk", "docs get")?)
    } else {
        None
    };

    Ok(KnowledgeDetail {
        document: field(&data, "document", "docs get")?,
        sections: field(&data, "sections", "docs get")?,
        chunk,
    })
}

/// Replaces the links of document `id` (`docs links set --json ID`, the links as JSON on stdin);
/// none makes it global. Answers the links as stored.
pub async fn set_links(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: i64,
    links: &[KnowledgeLink],
) -> Result<Vec<KnowledgeLink>> {
    check_links(links)?;

    let mut args = words(&["docs", "links", "set", "--json"]);
    args.push(check_id(id)?.to_string().into());

    let stdin = serde_json::to_vec(links).map_err(|error| BusinessError::Internal(error.to_string()))?;
    let command = CliCall {
        name: "docs links set",
        args,
        timeout: LIST_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin: Some(stdin),
    };

    let data = run_json(env, runner, server, command).await?;

    field(&data, "links", "docs links set")
}

/// Renames document `id` (`docs rename --json ID --title TITLE`); answers the title as stored.
pub async fn rename_doc(env: &Env, runner: &dyn ProcessRunner, server: &Path, id: i64, title: &str) -> Result<String> {
    let title = title.trim();

    if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
        return Err(invalid(&format!("a title has 1 to {MAX_TITLE_CHARS} characters")));
    }

    let mut args = words(&["docs", "rename", "--json"]);
    args.extend([check_id(id)?.to_string().into(), "--title".into(), title.into()]);

    let data = run_json(env, runner, server, call("docs rename", args)).await?;

    field(&data, "title", "docs rename")
}

/// Deletes the documents `ids` with their chunks (`docs delete --json ID...`, 1 to 1000 ids).
pub async fn delete_docs(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    ids: &[i64],
) -> Result<KnowledgeDeleted> {
    if ids.is_empty() || ids.len() > MAX_DELETE_IDS {
        return Err(invalid(&format!("delete 1 to {MAX_DELETE_IDS} documents at a time")));
    }

    let mut args = words(&["docs", "delete", "--json"]);

    for id in ids {
        args.push(check_id(*id)?.to_string().into());
    }

    let data = run_json(env, runner, server, call("docs delete", args)).await?;
    tracing::info!(deleted = count_of(data.get("deleted")), "knowledge documents deleted");

    Ok(KnowledgeDeleted {
        deleted: count_of(data.get("deleted")),
    })
}

/// The repositories with documents and how many are global (`repos --json`). Never fails: see the
/// module docs.
pub async fn list_repos(env: &Env, runner: &dyn ProcessRunner, features: &KnowledgeFeatures) -> KnowledgeReposPage {
    let server = match docs_server(env, runner, features).await {
        Ok(server) => server,
        Err(error) => return KnowledgeReposPage::unavailable(availability_of(error)),
    };

    let answer = run_typed(env, runner, &server, call("repos", words(&["repos", "--json"]))).await;

    let page = match answer {
        Ok(data) => read_repos(&data).map_err(CliFailure::from),
        Err(failure) => Err(failure),
    };

    page.unwrap_or_else(|failure| KnowledgeReposPage::unavailable(availability_of_failure(failure)))
}

/// The page in the answer of `repos --json`.
fn read_repos(data: &Value) -> Result<KnowledgeReposPage> {
    Ok(KnowledgeReposPage {
        availability: KnowledgeAvailability::Available,
        global: field(data, "global", "repos")?,
        repos: field::<Vec<KnowledgeRepoCount>>(data, "repos", "repos")?,
    })
}

/// What the database holds (`stats --json`).
pub async fn read_stats(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Result<KnowledgeStats> {
    let data = run_json(env, runner, server, call("stats", words(&["stats", "--json"]))).await?;

    serde_json::from_value(data)
        .map_err(|_| BusinessError::Protocol("harness-knowledge stats answered counts this app cannot read".to_owned()))
}

/// `id` when it is a document or chunk id (a positive number).
fn check_id(id: i64) -> Result<i64> {
    if id >= 1 {
        Ok(id)
    } else {
        Err(invalid("that is not a document id"))
    }
}

/// A format name: lowercase letters, digits, `-` and `_`, 1 to 32 characters.
fn check_format(format: &str) -> Result<()> {
    let fine = (1..=32).contains(&format.len())
        && format
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');

    if fine {
        Ok(())
    } else {
        Err(invalid("that is not a format name"))
    }
}

fn check_repo_key(key: &str) -> Result<()> {
    if is_repo_key(key) {
        Ok(())
    } else {
        Err(invalid("that is not a repository key"))
    }
}

/// The links the server would take: at most 32, each with a repository key and at most 32 globs
/// of at most 256 characters without control characters.
pub(super) fn check_links(links: &[KnowledgeLink]) -> Result<()> {
    if links.len() > MAX_LINKS {
        return Err(invalid(&format!(
            "a document links to at most {MAX_LINKS} repositories"
        )));
    }

    for link in links {
        check_repo_key(&link.repo)?;

        let fine = link.globs.len() <= MAX_LINKS
            && link.globs.iter().all(|glob| {
                !glob.is_empty() && glob.chars().count() <= MAX_GLOB_CHARS && !glob.chars().any(char::is_control)
            });

        if !fine {
            return Err(invalid(&format!(
                "a link has at most {MAX_LINKS} path globs of at most {MAX_GLOB_CHARS} characters"
            )));
        }
    }

    Ok(())
}

fn invalid(message: &str) -> BusinessError {
    BusinessError::InvalidInput(message.to_owned())
}
