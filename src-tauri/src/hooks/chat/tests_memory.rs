//! A chat's first message says saving is off only where Harness Memory takes part: Project memory
//! on and a server that applies save guards. Elsewhere it saves as before, or not at all, and the
//! file of save guards (always written at launch) must say nothing.

use std::sync::Arc;

use harness_business::ecosystem::{ensure_global, memory_data_dir, update_global};
use harness_business::hooks::{ChatContext, HookSet, MemoryGuardState, SaveRule, SendOrigin, compose_prompt};
use harness_business::model::{AgentAction, HarnessId, MemoryGuardKind, MemoryGuardsPatch};
use tauri::async_runtime::block_on;

use super::ChatHooks;
use super::counts::Counts;
use super::memory::MemoryInputs;
use crate::state::AppState;
use crate::test_support::{TempDir, memory_data_setup, place_memory_server};

const NOTICE: &str = "Memory settings";

/// A state whose Harness Memory data folder holds the fresh guards file of a launch (saving off
/// until the user turns it on).
fn launched(label: &str) -> (TempDir, AppState) {
    let (dir, env, data) = memory_data_setup(label);
    std::fs::create_dir_all(env.home.clone()).expect("home");
    ensure_global(&data).expect("the guards file");

    let state = AppState::new(dir.path().join("config"), env);

    (dir, state)
}

fn chat() -> ChatHooks {
    let folder = std::env::temp_dir().join("opt-in-project");
    let context = ChatContext {
        session_id: "chat".to_owned(),
        folder: folder.clone(),
        project: Some(folder.to_string_lossy().into_owned()),
        fresh: true,
        utility: false,
        useful_skills: false,
    };

    ChatHooks::new(
        context,
        HarnessId::Omp,
        Some("3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b".to_owned()),
        Arc::new(Counts::default()),
        Arc::default(),
    )
}

/// The guards the chat reads now and whether its first message carries the memory notice.
fn first_message(state: &AppState) -> (MemoryGuardState, bool) {
    let chat = chat();
    let read = block_on(chat.memory_guards(&MemoryInputs::of(state)));
    let guards = read.unwrap_or(MemoryGuardState::OPEN);
    let action = AgentAction::Prompt {
        message: "Hello".to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    };

    let mut session = block_on(chat.session.lock());
    session.set_memory(read);
    let plan = session.on_send(&HookSet::empty(), &action, SendOrigin::User);
    let noticed = plan.memory_notice.as_ref().is_some_and(|part| part.hook_name == NOTICE);

    (guards, noticed)
}

fn features(state: &AppState, listed: &str) {
    std::fs::write(state.env.home.join(".fake-memory-features"), listed).expect("features");
}

fn project_memory(state: &AppState, enabled: bool) {
    state
        .prefs
        .with(|store| store.update(|prefs| prefs.memory_enabled = enabled))
        .expect("pref stored");
}

#[test]
fn a_chat_with_project_memory_off_hears_nothing() {
    let (dir, state) = launched("opt-in-pref-off");

    if place_memory_server(&dir.path().join("home")).is_none() {
        return;
    }

    features(&state, "save-guards,save-opt-in");
    project_memory(&state, false);

    assert_eq!(first_message(&state), (MemoryGuardState::OPEN, false));
}

#[test]
fn a_chat_without_harness_memory_installed_hears_nothing() {
    let (_dir, state) = launched("opt-in-not-installed");

    assert_eq!(first_message(&state), (MemoryGuardState::OPEN, false));
}

#[test]
fn a_chat_on_a_server_without_save_guards_hears_nothing() {
    let (dir, state) = launched("opt-in-no-guards");

    if place_memory_server(&dir.path().join("home")).is_none() {
        return;
    }

    features(&state, "embedder");
    assert_eq!(first_message(&state), (MemoryGuardState::OPEN, false));
}

#[test]
fn a_chat_on_a_server_with_save_guards_hears_that_saving_is_off() {
    let (dir, state) = launched("opt-in-guards");

    if place_memory_server(&dir.path().join("home")).is_none() {
        return;
    }

    let off = MemoryGuardState {
        notes: SaveRule::ProjectOff,
        scripts: SaveRule::ProjectOff,
    };

    for listed in ["save-guards,save-opt-in", "save-guards"] {
        features(&state, listed);
        let state = AppState::new(dir.path().join("config"), (*state.env).clone());

        assert_eq!(first_message(&state), (off, true), "{listed}");
    }
}

/// Sends `chat` a message as the shell does (the guards read first, the notice composed in front of
/// it and counted as delivered); whether the message carried the memory notice.
fn message_noticed(chat: &ChatHooks, state: &AppState) -> bool {
    let read = block_on(chat.memory_guards(&MemoryInputs::of(state)));
    let action = AgentAction::Prompt {
        message: "Go on".to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    };

    let mut session = block_on(chat.session.lock());
    session.set_memory(read);

    let mut plan = session.on_send(&HookSet::empty(), &action, SendOrigin::User);
    let parts = plan.context_parts(Vec::new());
    let (_, delivered) = compose_prompt("Go on", &parts);
    session.context_composed(&plan, &parts, &delivered);
    session.sending(&mut plan);

    plan.memory_notice.as_ref().is_some_and(|part| part.hook_name == NOTICE)
}

#[test]
fn project_memory_turned_off_and_on_again_says_only_what_changed() {
    let (dir, state) = launched("opt-in-toggle");

    if place_memory_server(&dir.path().join("home")).is_none() {
        return;
    }

    features(&state, "save-guards,save-opt-in");
    project_memory(&state, true);
    let chat = chat();

    assert!(
        message_noticed(&chat, &state),
        "saving is off for the project: told once"
    );
    assert!(!message_noticed(&chat, &state), "and not again");

    // Off mid-chat: Harness Memory takes no part, so there is nothing to say, not "on again".
    project_memory(&state, false);
    assert!(!message_noticed(&chat, &state));
    assert!(!message_noticed(&chat, &state));

    // On again with nothing changed since the agent was told: still nothing.
    project_memory(&state, true);
    assert!(!message_noticed(&chat, &state));

    // A change made while it was off is told once it takes part again.
    project_memory(&state, false);
    let data = memory_data_dir(&state.env).expect("data folder");
    let notes_everywhere = MemoryGuardsPatch {
        kind: MemoryGuardKind::Notes,
        paused: None,
        every_project: Some(true),
        add_repo: None,
        remove_repo: None,
    };
    update_global(&data, &notes_everywhere, None).expect("notes on");
    assert!(!message_noticed(&chat, &state));

    project_memory(&state, true);
    assert!(message_noticed(&chat, &state), "the change is told");
    assert!(!message_noticed(&chat, &state));
}
