//! One run of a Harness family server's command for the user ([`run_sidecar_cli`],
//! [`run_sidecar_text`], [`run_sidecar_json`]), shared by [`memory_cli`](super::memory_cli) and
//! every other product, and the page of a list ([`ListWindow`]) with the repositories a list reads
//! ([`Repos`]).
//!
//! The server runs in the home directory with closed stdin (`hooks put` gets the hook on it),
//! `NO_COLOR=1`, a time limit and capped
//! output. When it fails, the line that says why is the last one it prefixes with
//! `<binary>:` (it prints that one last, after any warning), else the last line that is not
//! empty, stderr before stdout; it comes back through [`redact_text`], at most 500 characters. A
//! usage error (exit 2) means the server does not know the command or an option: it predates it.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::sidecar::Sidecar;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{AuthorCount, AuthorFacet, MemoryRepos, SortOrder, is_clean_author_name};
use crate::process::{ExecSpec, ProcessRunner, launcher_search_dirs};
use crate::security::redact_text;

/// How long a list or read command may take (15 s).
pub const LIST_TIMEOUT: Duration = Duration::from_secs(15);

/// The output kept of each stream of a list or read command (1 MiB: a page of 200 entries,
/// or a note with its links, is far less).
pub(crate) const LIST_OUTPUT_BYTES: usize = 1024 * 1024;

/// How long deleting every note or script may take (60 s: one write per store, but a team
/// database far away holds thousands).
pub const DELETE_ALL_TIMEOUT: Duration = Duration::from_secs(60);

/// The entries of a page when the UI does not say.
pub const DEFAULT_PAGE_SIZE: u32 = 50;

/// The most entries on a page.
pub const MAX_PAGE_SIZE: u32 = 200;

/// The longest search, in characters (the server's own limit).
pub const MAX_QUERY_CHARS: usize = 256;

/// The sorts that go newest or most first unless asked otherwise; other sorts go A to Z.
const DESCENDING_SORTS: [&str; 3] = ["updated", "useful", "lastUseful"];

/// Which page of a list to read, and the search that filters it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListWindow {
    offset: u32,
    limit: u32,
    query: Option<String>,
    order: Option<SortOrder>,
}

impl ListWindow {
    /// From offset 0 and [`DEFAULT_PAGE_SIZE`] entries unless given; a blank `query` is none.
    /// `invalidInput` for a limit of 0 or above [`MAX_PAGE_SIZE`], or a query longer than
    /// [`MAX_QUERY_CHARS`].
    pub fn new(
        offset: Option<u32>,
        limit: Option<u32>,
        query: Option<String>,
        order: Option<SortOrder>,
    ) -> Result<Self> {
        let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);

        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(BusinessError::InvalidInput(format!(
                "a page holds 1 to {MAX_PAGE_SIZE} entries"
            )));
        }

        let query = query
            .map(|query| query.trim().to_owned())
            .filter(|query| !query.is_empty());

        if query
            .as_ref()
            .is_some_and(|query| query.chars().count() > MAX_QUERY_CHARS)
        {
            return Err(BusinessError::InvalidInput(format!(
                "a search is at most {MAX_QUERY_CHARS} characters"
            )));
        }

        Ok(Self {
            offset: offset.unwrap_or(0),
            limit,
            query,
            order,
        })
    }

    /// `list --json --offset N --limit N [--query Q] <filters> --sort S --order O
    /// [--project P]`: `filters` are the command's own options and values (`--kind fix`).
    pub(crate) fn list_args(&self, filters: &[(&str, &str)], sort: &str, repos: Repos<'_>) -> Vec<OsString> {
        let order = order_name(self.descending(DESCENDING_SORTS.contains(&sort)));

        let mut args: Vec<OsString> = ["list", "--json", "--offset"].map(OsString::from).into();
        args.push(self.offset.to_string().into());
        args.push("--limit".into());
        args.push(self.limit.to_string().into());

        if let Some(query) = &self.query {
            args.push("--query".into());
            args.push(query.into());
        }

        for (option, value) in filters {
            args.push(option.into());
            args.push(value.into());
        }

        args.extend(["--sort", sort, "--order", order].map(OsString::from));
        args.extend(repos_args(repos));

        args
    }
}

impl ListWindow {
    /// True when the page is asked newest or most first: the order given, else `by_default` (what
    /// the sort of the list does unless told otherwise).
    pub(crate) fn descending(&self, by_default: bool) -> bool {
        match self.order {
            Some(SortOrder::Asc) => false,
            Some(SortOrder::Desc) => true,
            None => by_default,
        }
    }

    /// The search that filters the page, when there is one.
    pub(crate) fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    /// The page as a list that counts pages from 1 asks for it: `(page, per_page)`. `invalidInput`
    /// when the page does not start at a multiple of its size.
    pub(crate) fn numbered(&self) -> Result<(u32, u32)> {
        if !self.offset.is_multiple_of(self.limit) {
            return Err(BusinessError::InvalidInput(
                "a page starts at a multiple of its size".to_owned(),
            ));
        }

        Ok((self.offset / self.limit + 1, self.limit))
    }

    /// The entries of the page out of `items`, a list the server hands over whole (already
    /// filtered and in the order asked): from the offset, at most the limit.
    pub(crate) fn page_of<T>(&self, items: Vec<T>) -> Vec<T> {
        items
            .into_iter()
            .skip(usize::try_from(self.offset).unwrap_or(usize::MAX))
            .take(usize::try_from(self.limit).unwrap_or(usize::MAX))
            .collect()
    }
}

/// The server's name of a direction.
fn order_name(descending: bool) -> &'static str {
    if descending { "desc" } else { "asc" }
}

/// The most people of an authors facet that are read (the servers send at most 200).
const MAX_FACET_PEOPLE: usize = 201;

/// The authors facet of a list answer (`"authors": {"people": [{"name", "count"}], "unknown": N}`),
/// shared by Harness Memory's and Harness Knowledge's lists. `None` when the answer has no such
/// object, as from a server without the author filter. An entry that is not a name with a count is
/// skipped, and so is a name that cleaning would change (a control or invisible character, an edge
/// space, over the limit): it could not be given back as a filter. At most [`MAX_FACET_PEOPLE`]
/// entries are read.
pub(crate) fn author_facet(data: &Value) -> Option<AuthorFacet> {
    let facet = data.get("authors").filter(|facet| facet.is_object())?;

    let entries = facet
        .get("people")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);

    let people = entries
        .iter()
        .take(MAX_FACET_PEOPLE)
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str().filter(|name| is_clean_author_name(name))?;
            let count = entry.get("count")?.as_u64()?;

            Some(AuthorCount {
                name: name.to_owned(),
                count: u32::try_from(count).unwrap_or(u32::MAX),
            })
        })
        .collect();

    let unknown = facet.get("unknown").and_then(Value::as_u64).unwrap_or(0);

    Some(AuthorFacet {
        people,
        unknown: u32::try_from(unknown).unwrap_or(u32::MAX),
    })
}

/// The longest repository key accepted (a path or a remote; far more than any real one).
const MAX_KEY_CHARS: usize = 1024;

/// Which repositories a list command reads besides the global ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repos<'a> {
    /// `--project <root>`: that project's repository; none for the global ones only.
    Project(Option<&'a Path>),
    /// `--repo <key>`: the repository with that key, wherever it is checked out.
    Key(&'a str),
    /// `--all-repos`: every repository.
    All,
}

impl<'a> Repos<'a> {
    /// `repos` when the caller picked repositories, else the repository of `project`. `invalidInput` for a
    /// repository key that is not one (a real key starts with `git:` or `path:`, has no control characters, and
    /// is at most [`MAX_KEY_CHARS`] long), so no string from the UI can name anything but one repository.
    pub fn of(project: Option<&'a Path>, repos: Option<&'a MemoryRepos>) -> Result<Self> {
        match repos {
            Some(MemoryRepos::All) => Ok(Self::All),
            Some(MemoryRepos::Repo { key }) => {
                if is_repo_key(key) {
                    Ok(Self::Key(key))
                } else {
                    Err(BusinessError::InvalidInput("that is not a repository key".to_owned()))
                }
            }
            None => Ok(Self::Project(project)),
        }
    }
}

/// True for a repository key: it starts with `git:` or `path:`, has no control characters, and
/// is at most [`MAX_KEY_CHARS`] long.
pub(crate) fn is_repo_key(key: &str) -> bool {
    (key.starts_with("git:") || key.starts_with("path:"))
        && key.chars().count() <= MAX_KEY_CHARS
        && !key.chars().any(char::is_control)
}

/// The options that name `repos`: `--project <root>`, `--repo <key>` or `--all-repos`.
pub(crate) fn repos_args(repos: Repos<'_>) -> Vec<OsString> {
    match repos {
        Repos::Project(project) => project_args(project),
        Repos::Key(key) => vec!["--repo".into(), key.into()],
        Repos::All => vec!["--all-repos".into()],
    }
}

/// `--project <root>` when there is a project.
pub(crate) fn project_args(project: Option<&Path>) -> Vec<OsString> {
    match project {
        Some(project) => vec!["--project".into(), project.as_os_str().to_owned()],
        None => Vec::new(),
    }
}

/// The longest message returned, in characters.
const MAX_MESSAGE_CHARS: usize = 500;

/// The exit code of a usage error: the server does not know the command or one of its options.
pub(crate) const USAGE_EXIT: i32 = 2;

/// One command to run.
pub(crate) struct CliCall<'a> {
    /// The command's name in messages (`trust`, `notes list`).
    pub name: &'a str,
    /// The arguments after the binary.
    pub args: Vec<OsString>,
    /// How long it may take.
    pub timeout: Duration,
    /// The output kept of each stream.
    pub output_bytes: usize,
    /// What the command reads on stdin (`hooks put`); `None` for a closed stdin.
    pub stdin: Option<Vec<u8>>,
}

/// How a command that ran in time ended.
pub(crate) enum CliAnswer {
    /// It exited 0: its stdout.
    Done(String),
    /// It failed: its exit code (`None` when stopped by a signal) and the line that says why
    /// (redacted, clipped), or a sentence with the exit code when it printed nothing.
    Failed {
        /// The exit code.
        status: Option<i32>,
        /// What to show.
        message: String,
        /// Its stdout as printed (commands that answer a failure as JSON). Never log it.
        stdout: String,
    },
}

/// Runs `call` on the server of `sidecar` at `server`. `process` when it does not start or does
/// not answer in time.
pub(crate) async fn run_sidecar_cli(
    sidecar: &Sidecar,
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    call: CliCall<'_>,
) -> Result<CliAnswer> {
    let mut spec = ExecSpec::new(server)
        .args(call.args)
        .cwd(&env.home)
        .env("NO_COLOR", "1")
        .search_dirs(launcher_search_dirs(env, server))
        .timeout(call.timeout)
        .max_output_bytes(call.output_bytes);

    if let Some(stdin) = call.stdin {
        spec = spec.stdin(stdin);
    }

    let output = runner.run(&spec).await.map_err(|error| {
        tracing::warn!(%error, command = call.name, "{} could not be started", sidecar.binary);
        BusinessError::Process(format!("{} could not be started", sidecar.product))
    })?;

    if output.timed_out {
        return Err(BusinessError::Process(format!(
            "{} did not answer within {} seconds",
            sidecar.product,
            call.timeout.as_secs()
        )));
    }

    if output.success() {
        return Ok(CliAnswer::Done(output.stdout));
    }

    let prefix = format!("{}:", sidecar.binary);
    let decisive = decisive_line(&output.stderr, &prefix).or_else(|| decisive_line(&output.stdout, &prefix));

    let message = match decisive {
        Some(line) => clean(line),
        None => match output.status {
            Some(code) => format!("{} {} failed (exit code {code})", sidecar.binary, call.name),
            None => format!("{} {} was stopped", sidecar.binary, call.name),
        },
    };

    Ok(CliAnswer::Failed {
        status: output.status,
        message,
        stdout: output.stdout,
    })
}

/// The line of `text` that says why the server failed (see the module docs): the last one that
/// starts with the server's `prefix`.
fn decisive_line<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let lines = || text.lines().map(str::trim).filter(|line| !line.is_empty());

    lines()
        .rev()
        .find(|line| line.starts_with(prefix))
        .or_else(|| lines().next_back())
}

/// `line` through [`redact_text`], at most [`MAX_MESSAGE_CHARS`] long.
pub(crate) fn clean(line: &str) -> String {
    let redacted = redact_text(line);

    match redacted.char_indices().nth(MAX_MESSAGE_CHARS) {
        Some((end, _)) => format!("{}…", &redacted[..end]),
        None => redacted,
    }
}

/// Runs `call` and returns its stdout. `unsupported` for a usage error (the server predates the
/// command), `process` with the server's line when it fails otherwise, does not start or does not
/// answer in time.
pub(crate) async fn run_sidecar_text(
    sidecar: &Sidecar,
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    call: CliCall<'_>,
) -> Result<String> {
    let name = call.name.to_owned();

    match run_sidecar_cli(sidecar, env, runner, server, call).await? {
        CliAnswer::Done(stdout) => Ok(stdout),
        CliAnswer::Failed {
            status: Some(USAGE_EXIT),
            ..
        } => Err(BusinessError::Unsupported(format!(
            "this {} predates `{name}`; update {}",
            sidecar.binary, sidecar.product
        ))),
        CliAnswer::Failed { message, .. } => Err(BusinessError::Process(message)),
    }
}

/// [`run_sidecar_text`], its stdout read as JSON. `protocol` when it is not JSON (the parser's
/// message is dropped: it can quote the data).
pub(crate) async fn run_sidecar_json(
    sidecar: &Sidecar,
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    call: CliCall<'_>,
) -> Result<Value> {
    let name = call.name.to_owned();
    let stdout = run_sidecar_text(sidecar, env, runner, server, call).await?;

    serde_json::from_str(&stdout).map_err(|_| {
        BusinessError::Protocol(format!(
            "{} {name} answered output this app cannot read",
            sidecar.binary
        ))
    })
}
