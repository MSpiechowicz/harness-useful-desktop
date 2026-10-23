//! The local embedder behind vector search: `harness-memory embedder run` is one process per
//! computer that embeds notes and answers every `harness-memory mcp` over a local socket, and the
//! app keeps it running (see [`super::memory_embedder_supervisor`]).
//!
//! This file holds what the app understands of it:
//!
//! - the JSON lines it prints on stdout ([`EmbedderEvent`], read with [`parse_event`]) and the
//!   state they add up to ([`EmbedderTracker`]);
//! - its short commands: `embedder enable|disable` (the switch it keeps in its data folder),
//!   `embedder status --json`, `embedder remove-model`, and the store setting `settings get|set
//!   english_notes=… --json`. Each asks for the server through the `features` it lists
//!   ([`feature_server`]): `notFound` when Harness Memory is not installed, `unsupported` when it
//!   predates the feature.
//!
//! Nothing here logs what the embedder printed: only the phase it reached.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::catalog::MEMORY_BINARY;
use super::memory_binary::resolve_memory;
use super::memory_cli::{CliCall, DELETE_ALL_TIMEOUT, LIST_OUTPUT_BYTES, LIST_TIMEOUT, clean, run_json, run_text};
use super::memory_features::{FeaturesSupport, MemoryFeatures};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{EmbedderPhase, MemoryEmbedderState, MemorySettings};
use crate::process::ProcessRunner;

/// The most stores whose progress is followed at once (a computer has a handful).
const MAX_STORES: usize = 64;

/// One line the embedder printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbedderEvent {
    /// `download {received,total}`: bytes of the model so far.
    Download { received: u64, total: u64 },
    /// `verifying`: the downloaded model is being checked.
    Verifying,
    /// `loading`: the model is being loaded.
    Loading,
    /// `ready {model,dim}`: it serves searches with `model`.
    Ready { model: String },
    /// `backfill {store,done,total}`: the notes of `store` embedded so far.
    Backfill { store: String, done: u64, total: u64 },
    /// `model-update {from,to}`: a newer model is being fetched while `from` serves.
    ModelUpdate,
    /// `reembed {store,done,total}`: the notes of `store` embedded with the newer model so far.
    Reembed { store: String, done: u64, total: u64 },
    /// `switched {model}`: the newer model serves now.
    Switched { model: String },
    /// `cleanup-done`: the old model's files and vectors are gone.
    CleanupDone,
    /// `idle`: the model was unloaded after a quiet while, it loads again on demand.
    Idle,
    /// `error {message}`.
    Error { message: String },
    /// `busy`: another process already runs the embedder of this computer.
    Busy,
}

/// The event on one line of the embedder's stdout; `None` for anything this app does not
/// understand (a newer event, a line that is not JSON): such a line changes nothing.
pub fn parse_event(line: &str) -> Option<EmbedderEvent> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    let number = |name: &str| value.get(name).and_then(Value::as_u64).unwrap_or(0);
    let text = |name: &str| value.get(name).and_then(Value::as_str).map(str::to_owned);

    let event = match value.get("event")?.as_str()? {
        "download" => EmbedderEvent::Download {
            received: number("received"),
            total: number("total"),
        },
        "verifying" => EmbedderEvent::Verifying,
        "loading" => EmbedderEvent::Loading,
        "ready" => EmbedderEvent::Ready {
            model: model_name(text("model")),
        },
        "backfill" => EmbedderEvent::Backfill {
            store: text("store")?,
            done: number("done"),
            total: number("total"),
        },
        "model-update" => EmbedderEvent::ModelUpdate,
        "reembed" => EmbedderEvent::Reembed {
            store: text("store")?,
            done: number("done"),
            total: number("total"),
        },
        "switched" => EmbedderEvent::Switched {
            model: model_name(text("model")),
        },
        "cleanup-done" => EmbedderEvent::CleanupDone,
        "idle" => EmbedderEvent::Idle,
        "error" => EmbedderEvent::Error {
            message: text("message").unwrap_or_default(),
        },
        "busy" => EmbedderEvent::Busy,
        _ => return None,
    };

    Some(event)
}

/// The most characters of a model name that are kept.
const MAX_MODEL_CHARS: usize = 200;

/// A model name from the embedder's output: clipped to [`MAX_MODEL_CHARS`], then redacted and
/// clipped like an error line. Empty when it said none.
fn model_name(model: Option<String>) -> String {
    let model = model.unwrap_or_default();
    let end = model
        .char_indices()
        .nth(MAX_MODEL_CHARS)
        .map_or(model.len(), |(end, _)| end);

    clean(&model[..end])
}

/// What the embedder's events add up to: the [`MemoryEmbedderState`] shown, plus what is needed to
/// fold the next event in (the progress of each store, whether a newer model is on its way).
#[derive(Debug, Default)]
pub struct EmbedderTracker {
    state: MemoryEmbedderState,
    backfill: BTreeMap<String, (u64, u64)>,
    reembed: BTreeMap<String, (u64, u64)>,
    updating: bool,
    busy: bool,
}

impl EmbedderTracker {
    /// A tracker in the `off` state.
    pub fn new() -> Self {
        Self::default()
    }

    /// The state the events added up to.
    pub fn state(&self) -> &MemoryEmbedderState {
        &self.state
    }

    /// True once the embedder said `busy`: another process runs it.
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    /// A process is starting: `starting`, keeping the model it served and whether a model is
    /// downloaded, nothing else of its past.
    pub fn starting(&mut self) -> bool {
        self.reset(EmbedderPhase::Starting, None)
    }

    /// Whether a downloaded model is on this computer, as `embedder status` or a removal says.
    /// True when the state shown changed.
    pub fn model_present(&mut self, present: Option<bool>) -> bool {
        let changed = self.state.model_present != present;

        self.state.model_present = present;
        changed
    }

    /// The embedder is not running and is not meant to (`off`).
    pub fn stopped(&mut self) -> bool {
        self.reset(EmbedderPhase::Off, None)
    }

    /// The embedder was given up on (`failed`, with `error` redacted and clipped).
    pub fn failed(&mut self, error: &str) -> bool {
        self.reset(EmbedderPhase::Failed, Some(clean(error)))
    }

    fn reset(&mut self, phase: EmbedderPhase, error: Option<String>) -> bool {
        let before = self.state.clone();

        self.backfill.clear();
        self.reembed.clear();
        self.updating = false;
        self.busy = false;
        self.state = MemoryEmbedderState {
            phase,
            progress: None,
            model: self.state.model.clone().filter(|_| phase != EmbedderPhase::Off),
            error,
            model_present: self.state.model_present,
        };

        before != self.state
    }

    /// Folds `event` in. True when the state shown changed (progress counts in whole percents).
    pub fn apply(&mut self, event: EmbedderEvent) -> bool {
        let before = self.state.clone();

        match event {
            EmbedderEvent::Download { received, total } => {
                self.show(EmbedderPhase::Downloading, fraction(received, total));
            }
            EmbedderEvent::Verifying => self.show(EmbedderPhase::Verifying, None),
            EmbedderEvent::Loading => self.show(EmbedderPhase::Loading, None),
            EmbedderEvent::Ready { model } => {
                self.serve(model);
            }
            EmbedderEvent::Backfill { store, done, total } => {
                remember(&mut self.backfill, store, done, total);
                self.show_backfill();
            }
            EmbedderEvent::ModelUpdate => {
                self.updating = true;
                self.reembed.clear();
                self.show(EmbedderPhase::Updating, None);
            }
            EmbedderEvent::Reembed { store, done, total } => {
                self.updating = true;
                remember(&mut self.reembed, store, done, total);
                self.show(EmbedderPhase::Updating, total_fraction(&self.reembed));
            }
            EmbedderEvent::Switched { model } => {
                self.updating = false;
                self.reembed.clear();
                self.serve(model);
            }
            EmbedderEvent::CleanupDone => {}
            EmbedderEvent::Idle => {
                if self.state.model.is_some() && !self.updating {
                    self.settle();
                }
            }
            EmbedderEvent::Error { message } => self.fail(&message),
            EmbedderEvent::Busy => {
                self.busy = true;
                self.show(EmbedderPhase::Elsewhere, None);
            }
        }

        before != self.state
    }

    /// `model` serves searches now.
    fn serve(&mut self, model: String) {
        if !model.is_empty() {
            self.state.model = Some(model);
        }

        self.state.error = None;
        self.state.model_present = Some(true);
        self.settle();
    }

    /// Shows `phase` (an `updating` one while a newer model is on its way, the old model keeps
    /// serving) with `progress` rounded to a whole percent.
    fn show(&mut self, phase: EmbedderPhase, progress: Option<f64>) {
        let phase = match phase {
            EmbedderPhase::Downloading | EmbedderPhase::Verifying | EmbedderPhase::Loading if self.updating => {
                EmbedderPhase::Updating
            }
            other => other,
        };

        self.state.phase = phase;
        self.state.progress = progress.map(|fraction| (fraction * 100.0).round() / 100.0);
    }

    /// Shows the backfill so far, or `ready` once every store is through.
    fn show_backfill(&mut self) {
        if self.updating {
            return;
        }

        match total_fraction(&self.backfill) {
            Some(fraction) if fraction < 1.0 => self.show(EmbedderPhase::Backfilling, Some(fraction)),
            _ => self.settle(),
        }
    }

    /// `ready`, or `updating` while a newer model is on its way.
    fn settle(&mut self) {
        let phase = if self.updating {
            EmbedderPhase::Updating
        } else {
            EmbedderPhase::Ready
        };

        self.state.phase = phase;
        self.state.progress = if self.updating {
            total_fraction(&self.reembed)
        } else {
            None
        };
    }

    /// An error: a failed move to a newer model leaves the old one serving, anything else is
    /// `failed`.
    fn fail(&mut self, message: &str) {
        self.state.error = Some(clean(message)).filter(|message| !message.is_empty());

        if self.updating {
            self.updating = false;
            self.reembed.clear();
            self.settle();
        } else {
            self.state.phase = EmbedderPhase::Failed;
            self.state.progress = None;
        }
    }
}

/// `done` of `total` as a fraction from 0 to 1; `None` when the total is not known.
fn fraction(done: u64, total: u64) -> Option<f64> {
    if total == 0 {
        return None;
    }

    Some((done as f64 / total as f64).clamp(0.0, 1.0))
}

/// What the stores in `progress` have done together; `None` without any.
fn total_fraction(progress: &BTreeMap<String, (u64, u64)>) -> Option<f64> {
    let (done, total) = progress.values().fold((0_u64, 0_u64), |(done, total), (d, t)| {
        (done.saturating_add(*d), total.saturating_add(*t))
    });

    if progress.is_empty() {
        return None;
    }

    // Stores with nothing to embed are through.
    Some(fraction(done, total).unwrap_or(1.0))
}

/// Keeps what `store` has done, up to [`MAX_STORES`] stores.
fn remember(progress: &mut BTreeMap<String, (u64, u64)>, store: String, done: u64, total: u64) {
    if progress.len() < MAX_STORES || progress.contains_key(&store) {
        progress.insert(store, (done, total));
    }
}

/// The installed server when `feature` of its `features` is listed: `notFound` when Harness Memory
/// is not installed, `unsupported` (naming `what`) when it predates the feature.
pub async fn feature_server(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &MemoryFeatures,
    feature: fn(FeaturesSupport) -> bool,
    what: &str,
) -> Result<PathBuf> {
    let Some(server) = resolve_memory(env) else {
        return Err(BusinessError::NotFound(format!("executable {MEMORY_BINARY}")));
    };

    if !feature(features.support(env, runner).await) {
        return Err(BusinessError::Unsupported(format!(
            "this {MEMORY_BINARY} predates {what}; update Harness Memory"
        )));
    }

    Ok(server.path)
}

/// `embedder enable` or `embedder disable`: whether the embedder may run (the server keeps the
/// choice in its data folder, so its own tools agree with the app).
pub async fn set_embedder_enabled(env: &Env, runner: &dyn ProcessRunner, server: &Path, enabled: bool) -> Result<()> {
    let name = if enabled { "embedder enable" } else { "embedder disable" };

    run_text(
        env,
        runner,
        server,
        short_call(name, &[if enabled { "enable" } else { "disable" }]),
    )
    .await
    .map(drop)
}

/// What `embedder status --json` says the app needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbedderStatus {
    /// The switch of `embedder enable|disable` (off when the server does not say).
    pub enabled: bool,
    /// Whether a downloaded model is on this computer (`None` when the server does not say).
    pub model_present: Option<bool>,
}

/// `embedder status --json`. `protocol` when the answer is not JSON.
pub async fn embedder_status(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Result<EmbedderStatus> {
    let data = run_json(
        env,
        runner,
        server,
        short_call("embedder status", &["status", "--json"]),
    )
    .await?;

    Ok(EmbedderStatus {
        enabled: data.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        model_present: data.get("model_present").and_then(Value::as_bool),
    })
}

/// `embedder remove-model`: deletes the model files and the vectors (the server refuses while the
/// embedder runs).
pub async fn remove_embedder_model(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Result<()> {
    let mut call = short_call("embedder remove-model", &["remove-model"]);
    call.timeout = DELETE_ALL_TIMEOUT;

    run_text(env, runner, server, call).await.map(drop)
}

/// `settings get --json`: `english_notes` is on unless the server says otherwise.
pub async fn get_settings(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Result<MemorySettings> {
    let call = CliCall {
        name: "settings get",
        args: ["settings", "get", "--json"].map(OsString::from).into(),
        timeout: LIST_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin: None,
    };
    let data = run_json(env, runner, server, call).await?;

    Ok(read_settings(&data, true))
}

/// `settings set english_notes=<bool> --json`: the setting as the server answers it, else as asked.
pub async fn set_settings(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    english_notes: bool,
) -> Result<MemorySettings> {
    let assignment = format!("english_notes={english_notes}");
    let call = CliCall {
        name: "settings set",
        args: ["settings", "set", assignment.as_str(), "--json"]
            .map(OsString::from)
            .into(),
        timeout: LIST_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin: None,
    };
    let data = run_json(env, runner, server, call).await?;

    tracing::info!(english_notes, "memory setting changed");
    Ok(read_settings(&data, english_notes))
}

fn read_settings(data: &Value, fallback: bool) -> MemorySettings {
    MemorySettings {
        english_notes: data.get("english_notes").and_then(Value::as_bool).unwrap_or(fallback),
    }
}

/// A command under `embedder` with `args` after it.
fn short_call<'a>(name: &'a str, args: &[&str]) -> CliCall<'a> {
    let mut all: Vec<OsString> = vec!["embedder".into()];
    all.extend(args.iter().map(OsString::from));

    CliCall {
        name,
        args: all,
        timeout: LIST_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin: None,
    }
}
