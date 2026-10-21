//! Chats on the OpenCode 2 engine against a scripted OpenCode 2 server (`fake_agent --dialect opencode2`).

use std::path::Path;

use harness_business::model::HarnessId;

use super::{AgentSetup, ScenarioSpawner, agent_setup_with};

/// An app state with an opened project and OpenCode 2 (`2.1.2`) detected at `binary`: its chats run
/// on the OpenCode 2 engine and play the `scenario` stream as a scripted OpenCode 2 server.
pub fn agent_setup_opencode2(binary: &Path, scenario: &str) -> AgentSetup {
    let spawner = ScenarioSpawner::in_dialect(binary, "opencode2", scenario);
    let setup = agent_setup_with(binary, HarnessId::OpenCode, spawner);

    let opencode = setup
        .state
        .registry
        .get(HarnessId::OpenCode)
        .expect("OpenCode is registered");
    opencode.note_version(Some("2.1.2"));

    setup
}
