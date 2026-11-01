//! The local embedder behind vector search of the memory server: `harness-memory embedder run` is
//! one process per computer that embeds notes and answers every `harness-memory mcp` over a local
//! socket, and the app keeps it running (see [`super::sidecar_embedder_supervisor`]).
//!
//! What the app understands of it (the events, the state they add up to, the short commands) is
//! [`sidecar_embedder`](super::sidecar_embedder)'s, run for [`MEMORY_SIDECAR`]. This file adds the
//! memory server's own: each command asks for the server through the `features` it lists
//! ([`feature_server`]), `notFound` when Harness Memory is not installed, `unsupported` when it
//! predates the feature, and the store setting `settings get|set english_notes=… --json`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::catalog::MEMORY_SIDECAR;
use super::memory_cli::{CliCall, LIST_OUTPUT_BYTES, LIST_TIMEOUT, run_json};
use super::memory_features::{FeaturesSupport, MemoryFeatures};
pub use super::sidecar_embedder::{EmbedderEvent, EmbedderStatus, EmbedderTracker, parse_event};
use super::sidecar_embedder::{
    remove_sidecar_embedder_model, set_sidecar_embedder_enabled, sidecar_embedder_status, sidecar_feature_server,
};
use super::sidecar_embedder_supervisor::EmbedderFeatures;
use crate::env::Env;
use crate::error::Result;
use crate::model::MemorySettings;
use crate::process::ProcessRunner;

/// The installed server when `feature` of its `features` is listed: `notFound` when Harness Memory
/// is not installed, `unsupported` (naming `what`) when it predates the feature.
pub async fn feature_server(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &MemoryFeatures,
    feature: fn(FeaturesSupport) -> bool,
    what: &str,
) -> Result<PathBuf> {
    let listed = async { feature(features.support(env, runner).await) };

    sidecar_feature_server(&MEMORY_SIDECAR, env, listed, what).await
}

impl EmbedderFeatures for MemoryFeatures {
    async fn embedder_server(&self, env: &Env, runner: &dyn ProcessRunner) -> Result<PathBuf> {
        feature_server(env, runner, self, FeaturesSupport::embedder, "vector search").await
    }
}

/// `embedder enable` or `embedder disable` (see [`set_sidecar_embedder_enabled`]).
pub async fn set_embedder_enabled(env: &Env, runner: &dyn ProcessRunner, server: &Path, enabled: bool) -> Result<()> {
    set_sidecar_embedder_enabled(&MEMORY_SIDECAR, env, runner, server, enabled).await
}

/// `embedder status --json` (see [`sidecar_embedder_status`]).
pub async fn embedder_status(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Result<EmbedderStatus> {
    sidecar_embedder_status(&MEMORY_SIDECAR, env, runner, server).await
}

/// `embedder remove-model` (see [`remove_sidecar_embedder_model`]).
pub async fn remove_embedder_model(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Result<()> {
    remove_sidecar_embedder_model(&MEMORY_SIDECAR, env, runner, server).await
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
