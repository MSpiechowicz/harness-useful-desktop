//! The notices for command hooks that gave no output.

use harness_business::hooks::{CommandFailure, CommandFailureKind};
use harness_business::model::NoticeLevel;

use crate::agents::Agents;

/// Puts a notice in chat `key` for each command hook that gave no output.
pub(super) fn report(agents: &Agents, key: &str, failures: &[CommandFailure]) {
    for failure in failures {
        tracing::info!(session = %key, hook = failure.hook_id, kind = failure.kind.as_str(), "hook command gave no output");

        // A chat that is gone has nobody to tell.
        agents
            .manager()
            .notice(key, NoticeLevel::Warning, failure_notice(failure))
            .ok();
    }
}

/// The notice for `failure`: the hook's name and what went wrong, never its command or output.
fn failure_notice(failure: &CommandFailure) -> String {
    let name = &failure.hook_name;

    match failure.kind {
        CommandFailureKind::Spawn => format!("The hook \"{name}\" could not start its command."),
        CommandFailureKind::Exit(Some(code)) => format!("The hook \"{name}\" failed with exit code {code}."),
        CommandFailureKind::Exit(None) => format!("The hook \"{name}\" was stopped before it finished."),
        CommandFailureKind::Timeout => format!("The hook \"{name}\" timed out."),
        CommandFailureKind::Skipped => format!("The hook \"{name}\" was skipped, its event ran out of time."),
    }
}
