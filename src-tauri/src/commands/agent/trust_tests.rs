//! Claude Code folder trust in the chats the user opens: a folder Claude Code does not trust is
//! refused until the user trusts it once, then every later chat of that project in the same app
//! run (a restart, a reopened chat, the AI-naming helper) trusts it once too. A routine run never
//! does, and a fresh app start asks again.

use std::sync::Arc;

use harness_business::agent::{AgentManager, SessionOptions};
use harness_business::harnesses::claude::global_config_file;
use harness_business::model::{AgentStartRequest, HarnessId, IpcErrorCode};
use tauri::async_runtime::block_on;

use super::*;
use crate::test_support::{AgentSetup, agent_setup_for, collecting_channel, fake_agent, start_request};

/// A Claude Code chat setup whose test home trusts no folder; `None` when `fake_agent` is not
/// built.
fn untrusted_setup() -> Option<AgentSetup> {
    let binary = fake_agent()?;
    let setup = agent_setup_for(&binary, HarnessId::Claude, "claude_init.jsonl");
    std::fs::remove_file(global_config_file(&setup.state.env)).expect("no trusted folder");

    Some(setup)
}

fn request(setup: &AgentSetup, trust_once: bool, utility: bool) -> AgentStartRequest {
    AgentStartRequest {
        harness: HarnessId::Claude,
        trust_once,
        utility,
        ..start_request(&setup.project)
    }
}

/// Starts `request` as the user does; answers whether the chat trusted the project once, after
/// stopping it.
fn open(setup: &AgentSetup, request: AgentStartRequest) -> Result<bool, IpcErrorCode> {
    let (channel, _) = collecting_channel();
    let info = block_on(start(&setup.state, request, channel)).map_err(|error| error.code())?;
    block_on(stop(&setup.state, &info.key)).expect("stopped");

    Ok(info.trusted_once)
}

#[test]
fn a_folder_trusted_once_stays_trusted_for_the_projects_chats_until_the_app_quits() {
    let Some(mut setup) = untrusted_setup() else {
        return;
    };

    assert_eq!(
        open(&setup, request(&setup, false, false)),
        Err(IpcErrorCode::Untrusted)
    );
    assert_eq!(
        open(&setup, request(&setup, true, false)),
        Ok(true),
        "the user trusts it once"
    );

    assert_eq!(
        open(&setup, request(&setup, false, false)),
        Ok(true),
        "a restart or a reopened chat"
    );
    assert_eq!(
        open(&setup, request(&setup, false, true)),
        Ok(true),
        "the AI-naming helper"
    );

    let (channel, _) = collecting_channel();
    let sink = Arc::new(ChannelSink::new(
        channel,
        HarnessId::Claude,
        stop_when_closed(&setup.state.agents, &setup.state.hooks),
    ));
    let folder = AuthorizedFolder::of_routine(&setup.project);
    let routine = block_on(start_with(&setup.state, request(&setup, false, false), sink, folder))
        .expect_err("a routine run trusts nothing it did not ask for");
    assert_eq!(routine.code(), IpcErrorCode::Untrusted);

    let restarted = AgentManager::with_options(SessionOptions::default());
    setup.state.agents = Arc::new(Agents::with(restarted, setup.spawner.clone()));
    assert_eq!(
        open(&setup, request(&setup, false, false)),
        Err(IpcErrorCode::Untrusted),
        "a fresh app start asks again"
    );
    assert!(
        !global_config_file(&setup.state.env).exists(),
        "nothing written to Claude's files"
    );
}
