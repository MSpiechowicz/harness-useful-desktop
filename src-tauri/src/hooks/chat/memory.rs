//! A chat's memory guards as its hooks see them ([`ChatHooks::memory_guards`], read at each
//! settle and each user message): the memory-save hook ("Save what you learned") is skipped while
//! the chat is paused, or notes and scripts are both off for it, its repository or everywhere, and
//! asks about one kind only while the other is off. "Check again" sends that hook's text, as the
//! user keeps it and tailored the same way, at once through the hook path ([`check_again`]),
//! refused in those states.

use std::path::Path;
use std::sync::Arc;

use harness_business::ecosystem::{MemoryFeatures, memory_data_dir};
use harness_business::env::Env;
use harness_business::hooks::{
    HookSet, MemoryGuardState, SendOrigin, compose_follow_up, handled_without_run, memory_save_part,
    read_memory_guards, takes_part,
};
use harness_business::model::AgentAction;
use harness_business::process::TokioRunner;
use harness_business::{BusinessError, Result};

use super::ChatHooks;
use crate::state::{AppState, SharedPrefs};

/// What decides whether Harness Memory takes part in a chat ([`takes_part`]): taken from the app
/// state, so a chat's detached hook task can ask without it.
#[derive(Clone)]
pub(super) struct MemoryInputs {
    env: Arc<Env>,
    prefs: SharedPrefs,
    features: Arc<MemoryFeatures>,
}

impl MemoryInputs {
    pub(super) fn of(state: &AppState) -> Self {
        Self {
            env: state.env.clone(),
            prefs: state.prefs.clone(),
            features: state.memory_features.clone(),
        }
    }

    /// Whether Harness Memory takes part now: the "Project memory" pref is on and the installed
    /// server applies save guards (asked only while the pref is on, the answer is cached).
    async fn takes_part(&self) -> bool {
        let prefs = self.prefs.clone();
        let enabled = tauri::async_runtime::spawn_blocking(move || prefs.with(|store| store.load()).memory_enabled)
            .await
            .unwrap_or(false);

        enabled && takes_part(true, self.features.support(&self.env, &TokioRunner).await)
    }
}

impl ChatHooks {
    /// Where the memory guards in `inputs`' Harness Memory data folder leave this chat now. `None`
    /// while Harness Memory takes no part (Project memory off, not installed, or a server without
    /// save guards, which saves as before) or the folder is unknown: the chat is told nothing
    /// about saving then. Reads files off the async runtime.
    pub(super) async fn memory_guards(&self, inputs: &MemoryInputs) -> Option<MemoryGuardState> {
        let data_dir = memory_data_dir(&inputs.env)?;

        if !inputs.takes_part().await {
            return None;
        }

        let key = self.memory_key.clone();
        let repo = self.project();

        let read = tauri::async_runtime::spawn_blocking(move || {
            read_memory_guards(&data_dir, key.as_deref(), repo.as_deref().map(Path::new))
        })
        .await;

        Some(read.unwrap_or(MemoryGuardState::CLOSED))
    }

    /// The chat moved to `folder`: its memory guards file (when it has one) names the git root
    /// there, so Harness Memory checks the repository it works in now. A failure is logged.
    pub(super) async fn memory_moved(&self, state: &AppState, folder: &Path) {
        let (Some(key), Some(data_dir)) = (self.memory_key.clone(), memory_data_dir(&state.env)) else {
            return;
        };

        let writes = state.memory_chat_writes.clone();
        let folder = folder.to_path_buf();
        let written = tauri::async_runtime::spawn_blocking(move || writes.set_project(&data_dir, &key, &folder)).await;

        match written {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::warn!(code = ?error.code(), "the memory guards of a moved chat were not updated")
            }
            Err(error) => tracing::warn!(%error, "the memory guards of a moved chat were not updated"),
        }
    }
}

/// Sends the memory-save hook's text to chat `key` now, as a follow-up of the hooks (it starts no
/// hooks of its own): the text of that hook as it applies to the chat, edited or not, else the
/// catalog's while the chat has none, tailored to what the guards allow ([`memory_save_part`]).
/// `notFound` for a chat whose hooks are not tracked, `invalidInput` while its memory guards skip
/// that hook.
pub async fn check_again(state: &AppState, key: &str) -> Result<()> {
    let Some(chat) = state.hooks.chat(key) else {
        return Err(BusinessError::NotFound(format!("chat {key}")));
    };

    let guards = chat
        .memory_guards(&MemoryInputs::of(state))
        .await
        .unwrap_or(MemoryGuardState::OPEN);

    if guards.gate().skips_memory_save() {
        return Err(BusinessError::InvalidInput(
            "saving to Harness Memory is off for this chat".to_owned(),
        ));
    }

    let hooks = state.hooks.hook_set(chat.project().as_deref()).await;
    let part = memory_save_part(&hooks, guards);
    let Some(message) = compose_follow_up(&[part]) else {
        return Err(BusinessError::Internal(
            "the memory-save text does not fit a prompt".to_owned(),
        ));
    };

    let action = AgentAction::Prompt {
        message,
        images: Vec::new(),
        streaming_behavior: None,
    };

    // Held while the prompt goes out, as for a hook follow-up, so nothing overtakes it.
    let mut session = chat.session.lock().await;
    session.on_send(&HookSet::empty(), &action, SendOrigin::Hook);

    let settles = chat.settles_seen();
    let reply = state.agents.manager().send(key, action).await?;

    if !handled_without_run(&reply) {
        chat.counts.follow_up_sent(settles);
    }

    session.follow_up_replied(&reply);
    tracing::info!(session = %key, "memory save check sent");

    Ok(())
}
