//! What tells one Harness family server the app keeps (Harness Memory, Harness Knowledge) from
//! another in the app's own setup: its names, its prefs, the lock of its setup, its embedder and
//! the state of its MCP entry. [`crate::sidecar_setup`] and [`crate::sidecar_embedder`] do the work
//! for any [`Product`]; `memory_setup` and `knowledge_setup` add what only one has.

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use harness_business::ecosystem::catalog::{KNOWLEDGE_SIDECAR, MEMORY_SIDECAR};
use harness_business::ecosystem::knowledge_record::KnowledgeFile;
use harness_business::ecosystem::memory_record::MemoryFile;
use harness_business::ecosystem::sidecar::Sidecar;
use harness_business::ecosystem::{
    EmbedderFeatures, EmbedderSupervisor, KnowledgeFeatures, McpFile, MemoryFeatures, RecordFile, knowledge_mcp_state,
    memory_mcp_state,
};
use harness_business::env::Env;
use harness_business::model::{AppPrefs, McpState};
use tauri::async_runtime;

use crate::memory_embedder::EmbedderWatch;
use crate::state::AppState;

/// One Harness family server the app registers with the harnesses and keeps an embedder for.
pub(crate) trait Product: 'static {
    /// The server's names.
    const SIDECAR: &'static Sidecar;
    /// Where the files it was registered in are recorded.
    type File: RecordFile;
    /// What its `features` say about its embedder.
    type Features: EmbedderFeatures + 'static;

    /// Held while its server is installed, registered or unregistered, so one runs at a time.
    fn setup_lock(state: &AppState) -> &async_runtime::Mutex<()>;
    /// The pref that switches the product on (it registers the server).
    fn enabled(prefs: &AppPrefs) -> bool;
    /// Stores that pref as `enabled`.
    fn set_enabled(prefs: &mut AppPrefs, enabled: bool);
    /// The pref that switches vector search on.
    fn vector_search(prefs: &AppPrefs) -> bool;
    /// Stores that pref as `enabled`.
    fn set_vector_search(prefs: &mut AppPrefs, enabled: bool);
    /// The supervisor of its embedder.
    fn embedder(state: &AppState) -> &Arc<EmbedderSupervisor>;
    /// The page told when the embedder's state changes.
    fn watch(state: &AppState) -> &EmbedderWatch;
    /// The probe of its features.
    fn features(state: &AppState) -> &Self::Features;
    /// Its MCP entry in `file`, read-only.
    fn mcp_state(env: &Env, file: &McpFile) -> McpState;

    /// What a launch or an install does besides registering the server `server` (`None` when it is
    /// not installed): retires what older builds left behind. Nothing by default.
    fn retire_legacy(state: &AppState, server: Option<PathBuf>) -> impl Future<Output = ()> + Send {
        let _ = (state, server);

        async {}
    }
}

/// Harness Memory.
pub(crate) struct Memory;

impl Product for Memory {
    const SIDECAR: &'static Sidecar = &MEMORY_SIDECAR;
    type File = MemoryFile;
    type Features = MemoryFeatures;

    fn setup_lock(state: &AppState) -> &async_runtime::Mutex<()> {
        &state.memory_setup
    }

    fn enabled(prefs: &AppPrefs) -> bool {
        prefs.memory_enabled
    }

    fn set_enabled(prefs: &mut AppPrefs, enabled: bool) {
        prefs.memory_enabled = enabled;
    }

    fn vector_search(prefs: &AppPrefs) -> bool {
        prefs.vector_search
    }

    fn set_vector_search(prefs: &mut AppPrefs, enabled: bool) {
        prefs.vector_search = enabled;
    }

    fn embedder(state: &AppState) -> &Arc<EmbedderSupervisor> {
        &state.memory_embedder
    }

    fn watch(state: &AppState) -> &EmbedderWatch {
        &state.embedder_watch
    }

    fn features(state: &AppState) -> &MemoryFeatures {
        &state.memory_features
    }

    fn mcp_state(env: &Env, file: &McpFile) -> McpState {
        memory_mcp_state(env, file)
    }

    fn retire_legacy(state: &AppState, server: Option<PathBuf>) -> impl Future<Output = ()> + Send {
        crate::memory_setup::retire_legacy_copy(state, server)
    }
}

/// Harness Knowledge.
pub(crate) struct Knowledge;

impl Product for Knowledge {
    const SIDECAR: &'static Sidecar = &KNOWLEDGE_SIDECAR;
    type File = KnowledgeFile;
    type Features = KnowledgeFeatures;

    fn setup_lock(state: &AppState) -> &async_runtime::Mutex<()> {
        &state.knowledge_setup
    }

    fn enabled(prefs: &AppPrefs) -> bool {
        prefs.knowledge_enabled
    }

    fn set_enabled(prefs: &mut AppPrefs, enabled: bool) {
        prefs.knowledge_enabled = enabled;
    }

    fn vector_search(prefs: &AppPrefs) -> bool {
        prefs.knowledge_vector_search
    }

    fn set_vector_search(prefs: &mut AppPrefs, enabled: bool) {
        prefs.knowledge_vector_search = enabled;
    }

    fn embedder(state: &AppState) -> &Arc<EmbedderSupervisor> {
        &state.knowledge_embedder
    }

    fn watch(state: &AppState) -> &EmbedderWatch {
        &state.knowledge_embedder_watch
    }

    fn features(state: &AppState) -> &KnowledgeFeatures {
        &state.knowledge_features
    }

    fn mcp_state(env: &Env, file: &McpFile) -> McpState {
        knowledge_mcp_state(env, file)
    }
}
