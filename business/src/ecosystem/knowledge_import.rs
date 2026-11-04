//! The streamed import of documents: `harness-knowledge docs import --ndjson` reads a manifest on
//! stdin and prints one JSON event per line while it works ([`run_import`], `knowledge_import`).
//!
//! - One import runs at a time ([`KnowledgeImporter::begin`]): a second one is refused
//!   (`invalidInput`) until the first one ended.
//! - The manifest is `{"paths": [...], "folders": [...], "links": [...], "replaces": ID, "replacing":
//!   {PATH: ID}, "place": "remote"|"local", "draft": true, "ask": true, "note": TEXT}`, all optional
//!   (`replacing` only for a server that lists `docs-similar`, `draft`, `ask` and `note` only for
//!   one that lists `docs-drafts`), at most 4 MiB. It is written to the child's stdin, which is
//!   closed after it.
//! - Each stdout line is one event ([`parse_import_event`]); a line that is no event (a newer
//!   event, a stray line) is skipped. Every event goes to the sink as it arrives. An import that
//!   ends without its `done` event (a crash, a refused manifest) ends with a `failed` event that
//!   carries the server's last line (redacted).
//! - Cancel ([`KnowledgeImporter::cancel`]) asks the child to stop between two documents: SIGTERM
//!   on Unix, killing it on Windows, which has no such request. The files done before it stay.
//!   A child that has not ended [`CANCEL_GRACE`] after the request is killed.
//!
//! Paths and file names reach the sink and nothing else: they are never logged.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::sync::Notify;
use tokio::time::{Instant, sleep_until, timeout};

use super::catalog::KNOWLEDGE_SIDECAR;
pub use super::knowledge_manifest::{ImportManifest, MANIFEST_MAX_BYTES};
use super::sidecar_cli::clean;
use super::sidecar_embedder_lines::next_line;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    KnowledgeEmbedding, KnowledgeFileStatus, KnowledgeId, KnowledgeImportCounts, KnowledgeImportEvent,
    KnowledgeImportResult,
};
use crate::process::{ChildHandle, ExecSpec, ProcessSpawner, SpawnedProcess, launcher_search_dirs};

/// How long a child gets to end after it was asked to stop before it is killed.
pub const CANCEL_GRACE: Duration = Duration::from_secs(30);

/// How long the child gets to exit once its output ended.
const EXIT_GRACE: Duration = Duration::from_secs(5);

/// The longest stderr line kept for a failure message.
const STDERR_LINE_CHARS: usize = 500;

/// The event on one line of the import's stdout; `None` for anything this app does not understand.
pub fn parse_import_event(line: &str) -> Option<KnowledgeImportEvent> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    let number = |value: &Value, name: &str| {
        value
            .get(name)
            .and_then(Value::as_u64)
            .map_or(0, |count| u32::try_from(count).unwrap_or(u32::MAX))
    };

    match value.get("type")?.as_str()? {
        "plan" => Some(KnowledgeImportEvent::Plan {
            total: number(&value, "total"),
        }),
        "progress" => Some(KnowledgeImportEvent::Progress {
            done: number(&value, "done"),
            total: number(&value, "total"),
        }),
        "file" => Some(KnowledgeImportEvent::File {
            path: value.get("path")?.as_str()?.to_owned(),
            status: file_status(value.get("status")?.as_str()?)?,
            reason: value.get("reason").and_then(Value::as_str).map(clean),
            doc_id: value
                .get("docId")
                .and_then(|id| serde_json::from_value::<KnowledgeId>(id.clone()).ok())
                .filter(KnowledgeId::is_valid),
            chunks: value.get("chunks").map(|_| number(&value, "chunks")),
            embedding: match value.get("embedding").and_then(Value::as_str) {
                Some("done") => Some(KnowledgeEmbedding::Done),
                Some("pending") => Some(KnowledgeEmbedding::Pending),
                _ => None,
            },
            warnings: value
                .get("warnings")
                .and_then(Value::as_array)
                .map(|warnings| warnings.iter().filter_map(Value::as_str).map(clean).collect())
                .unwrap_or_default(),
            draft: value.get("draft").and_then(Value::as_bool).unwrap_or(false),
            asked: value.get("asked").and_then(Value::as_bool).unwrap_or(false),
        }),
        "done" => {
            let counts = value.get("counts").cloned().unwrap_or(Value::Null);

            Some(KnowledgeImportEvent::Done {
                counts: KnowledgeImportCounts {
                    added: number(&counts, "added"),
                    updated: number(&counts, "updated"),
                    pending: number(&counts, "pending"),
                    unchanged: number(&counts, "unchanged"),
                    skipped: number(&counts, "skipped"),
                    failed: number(&counts, "failed"),
                },
                cancelled: value.get("cancelled").and_then(Value::as_bool).unwrap_or(false),
                counter: value
                    .get("counter")
                    .and_then(Value::as_str)
                    .map_or_else(String::new, clean),
                embedding_pending: number(&value, "embeddingPending"),
            })
        }
        _ => None,
    }
}

fn file_status(name: &str) -> Option<KnowledgeFileStatus> {
    match name {
        "added" => Some(KnowledgeFileStatus::Added),
        "updated" => Some(KnowledgeFileStatus::Updated),
        "pending" => Some(KnowledgeFileStatus::Pending),
        "unchanged" => Some(KnowledgeFileStatus::Unchanged),
        "skipped" => Some(KnowledgeFileStatus::Skipped),
        "failed" => Some(KnowledgeFileStatus::Failed),
        _ => None,
    }
}

/// The one import that may run, and the way to stop it.
#[derive(Debug, Default)]
pub struct KnowledgeImporter {
    running: Mutex<Option<Arc<Notify>>>,
    grace: Option<Duration>,
}

/// Proof that an import runs: it holds the slot until dropped.
#[derive(Debug)]
pub struct ImportRun<'a> {
    importer: &'a KnowledgeImporter,
    cancel: Arc<Notify>,
    grace: Duration,
}

impl KnowledgeImporter {
    /// No import runs.
    pub fn new() -> Self {
        Self::default()
    }

    /// No import runs, and a cancelled child that does not stop is killed after `grace` instead of
    /// [`CANCEL_GRACE`].
    pub fn with_grace(grace: Duration) -> Self {
        Self {
            grace: Some(grace),
            ..Self::default()
        }
    }

    /// Takes the slot of the one import. `invalidInput` while another import runs.
    pub fn begin(&self) -> Result<ImportRun<'_>> {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);

        if running.is_some() {
            return Err(BusinessError::InvalidInput("an import is already running".to_owned()));
        }

        let cancel = Arc::new(Notify::new());
        *running = Some(cancel.clone());

        Ok(ImportRun {
            importer: self,
            cancel,
            grace: self.grace.unwrap_or(CANCEL_GRACE),
        })
    }

    /// Asks the running import to stop. False when none runs.
    pub fn cancel(&self) -> bool {
        let running = self.running.lock().unwrap_or_else(PoisonError::into_inner);

        match running.as_ref() {
            Some(cancel) => {
                // A permit is kept, so a cancel before the import listens still stops it.
                cancel.notify_one();
                true
            }
            None => false,
        }
    }

    /// Whether an import runs.
    pub fn is_running(&self) -> bool {
        self.running.lock().unwrap_or_else(PoisonError::into_inner).is_some()
    }
}

impl Drop for ImportRun<'_> {
    fn drop(&mut self) {
        *self.importer.running.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// Runs `harness-knowledge docs import --ndjson` at `server` with `manifest`, handing each event to
/// `sink` as it arrives (see the module docs). `run` is the slot [`KnowledgeImporter::begin`] gave.
/// Errors only when the child could not be started or its stdin written; an import that failed
/// later is a `failed` event and a result without counts.
pub async fn run_import(
    env: &Env,
    spawner: &dyn ProcessSpawner,
    server: &Path,
    manifest: &ImportManifest,
    run: &ImportRun<'_>,
    mut sink: impl FnMut(KnowledgeImportEvent),
) -> Result<KnowledgeImportResult> {
    let manifest = manifest.to_json()?;
    let spec = ExecSpec::new(server)
        .args(["docs", "import", "--ndjson"])
        .cwd(&env.home)
        .env("NO_COLOR", "1")
        .search_dirs(launcher_search_dirs(env, server));

    let SpawnedProcess {
        mut stdin,
        stdout,
        stderr,
        mut child,
    } = spawner.spawn(&spec).map_err(|error| {
        tracing::warn!(%error, "harness-knowledge could not be started for an import");
        BusinessError::Process(format!("{} could not be started", KNOWLEDGE_SIDECAR.product))
    })?;

    // The manifest is read to its end, so closing stdin is what tells the server it is complete.
    let written = async {
        stdin.write_all(&manifest).await?;
        stdin.shutdown().await
    };

    if let Err(error) = written.await {
        child.start_kill().ok();
        tracing::warn!(%error, "the import manifest could not be written");
        return Err(BusinessError::Process(format!(
            "{} stopped before it read the files to import",
            KNOWLEDGE_SIDECAR.product
        )));
    }

    drop(stdin);
    tracing::info!("knowledge import started");

    let last_error = tokio::spawn(last_line(stderr));
    let ended = read_events(stdout, &mut child, &run.cancel, run.grace, &mut sink).await;
    let exit = timeout(EXIT_GRACE, child.wait()).await;

    if exit.is_err() {
        child.start_kill().ok();
    }

    let result = match ended.result {
        Some(result) => result,
        None => {
            let message = match last_error.await.ok().flatten() {
                Some(line) if !ended.asked_to_stop => line,
                _ if ended.asked_to_stop => format!("{} stopped before the import was done", KNOWLEDGE_SIDECAR.binary),
                _ => format!("{} ended without finishing the import", KNOWLEDGE_SIDECAR.binary),
            };
            sink(KnowledgeImportEvent::Failed { message });

            KnowledgeImportResult {
                counts: None,
                cancelled: ended.asked_to_stop,
            }
        }
    };

    tracing::info!(cancelled = result.cancelled, "knowledge import ended");

    Ok(result)
}

/// How the child's output ended.
struct Ended {
    /// The result of the closing `done` event, none when it never came.
    result: Option<KnowledgeImportResult>,
    /// Whether a cancel was asked for.
    asked_to_stop: bool,
}

/// Reads the child's events until its output ends. A cancel asks the child to stop once and, after
/// [`CANCEL_GRACE`], kills it.
async fn read_events(
    stdout: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
    child: &mut Box<dyn ChildHandle>,
    cancel: &Notify,
    grace: Duration,
    sink: &mut impl FnMut(KnowledgeImportEvent),
) -> Ended {
    let mut reader = BufReader::new(stdout);
    let mut buffer = Vec::new();
    let mut result = None;
    let mut asked_to_stop = false;
    let mut kill_at = None;

    loop {
        // Only awaited once a cancel set the deadline.
        let due = kill_at.unwrap_or_else(|| Instant::now() + grace);

        tokio::select! {
            line = next_line(&mut reader, &mut buffer) => match line {
                Ok(Some(line)) => {
                    let Some(event) = parse_import_event(&line) else {
                        continue;
                    };

                    if let KnowledgeImportEvent::Done { counts, cancelled, .. } = &event {
                        result = Some(KnowledgeImportResult { counts: Some(*counts), cancelled: *cancelled });
                    }

                    sink(event);
                }
                Ok(None) | Err(_) => break,
            },
            () = cancel.notified(), if !asked_to_stop => {
                asked_to_stop = true;
                tracing::info!("knowledge import cancel requested");
                child.terminate();
                kill_at = Some(Instant::now() + grace);
            },
            () = sleep_until(due), if kill_at.is_some() => {
                tracing::warn!("the knowledge import did not stop in time and was killed");
                child.start_kill().ok();
                kill_at = None;
            },
        }
    }

    Ended { result, asked_to_stop }
}

/// The last non-empty line of `stderr`, redacted and clipped; `None` when it printed nothing. The
/// stream is read to its end so the child never blocks on a full pipe.
async fn last_line(stderr: Box<dyn tokio::io::AsyncRead + Send + Unpin>) -> Option<String> {
    let mut reader = BufReader::new(stderr);
    let mut buffer = Vec::new();
    let mut last = None;

    while let Ok(Some(line)) = next_line(&mut reader, &mut buffer).await {
        let line = line.trim();

        if !line.is_empty() {
            last = Some(line.to_owned());
        }
    }

    last.map(|line| {
        let line = line.strip_prefix("harness-knowledge:").map_or(line.as_str(), str::trim);
        let cleaned = clean(line);

        cleaned.chars().take(STDERR_LINE_CHARS).collect()
    })
}
