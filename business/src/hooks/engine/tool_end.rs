//! `toolEnd`: what a tool call that ended adds to the current run.

use super::{EventOutcome, Firing, HookSession, HookSet, PlannedPart};
use crate::hooks::plan::{ToolEnv, matches_tool, planned_part};
use crate::model::{HookAction, HookEvent};

impl HookSession {
    /// A tool call ended: it counts for the run's follow-up, its `followUp` texts are collected
    /// once per run, and its matching command hooks run now.
    pub(super) fn tool_ended(
        &mut self,
        hooks: &HookSet,
        name: &str,
        is_error: bool,
        touched: &[String],
    ) -> EventOutcome {
        self.current.touched.extend(touched.iter().cloned());

        for path in touched {
            self.chat_touched.add(path);
        }

        if !is_error {
            self.current.tool_succeeded = true;
        }

        // A run the engine's own prompt started fires nothing.
        if !self.current.user_sent() {
            return EventOutcome::Nothing;
        }

        let tool = ToolEnv {
            tool: Some((name, is_error)),
            touched,
        };
        let mut commands = Vec::new();

        for entry in hooks.of_event(HookEvent::ToolEnd) {
            let hook = &entry.hook;

            if !matches_tool(hook.matcher.as_deref(), name) {
                continue;
            }

            match &hook.action {
                HookAction::FollowUp { .. } => {
                    let seen = self.current.tool_texts.iter().any(|part| part.hook_id == hook.id);

                    if let (false, PlannedPart::Text(part)) =
                        (seen, planned_part(entry, &self.chat, HookEvent::ToolEnd, tool))
                    {
                        self.current.tool_texts.push(part);
                    }
                }
                HookAction::Command { .. } => {
                    commands.push(planned_part(entry, &self.chat, HookEvent::ToolEnd, tool));
                }
                HookAction::AddContext { .. } => {}
            }
        }

        if commands.is_empty() {
            return EventOutcome::Nothing;
        }

        self.current.outstanding += 1;

        EventOutcome::ToolCommands(Firing {
            event: HookEvent::ToolEnd,
            parts: commands,
            run: self.current.id,
            seq: self.take_seq(),
        })
    }
}
