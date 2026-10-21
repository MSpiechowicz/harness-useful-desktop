//! Which hooks fire in one chat: a pure, synchronous state machine with no IO.
//!
//! The shell feeds it every action it sends ([`HookSession::on_send`], then
//! [`HookSession::sending`] right before a user message goes out) and every agent event
//! ([`HookSession::on_event`]), and carries out what comes back: context to put in front of a
//! prompt, commands to run, a follow-up prompt to send. It passes the current [`HookSet`] each
//! time, so edits and approvals apply at once.
//!
//! - `sessionStart` fires on the first eligible prompt of a fresh chat and again after a
//!   `NewSession`; switching, forking or cloning a session disarms it. Eligible: a user `Prompt`
//!   without a streaming behaviour that does not start with `/` or `!`.
//! - `promptSubmit` fires on every eligible prompt.
//! - `toolEnd` fires on every matching `ToolEnd` of a run a user message started. A `followUp`
//!   hook contributes once per run; a command hook runs on each match and its latest output is
//!   what it contributes.
//! - `turnEnd` fires on `Settled` (not on each model turn) of every run a user message started:
//!   its command hooks always run. Follow-ups (`turnEnd` texts and command output, and the
//!   collected `toolEnd` ones) go out as one prompt, and only when at least one tool call ended
//!   without error, the run was not aborted and its prompt did not fail. A prompt the engine
//!   sends never counts as a user message, so its own run fires nothing, and a user message sent
//!   before it goes out cancels it. A user message the harness handled without a run (a
//!   `PromptResult` with `Handled`, e.g. a `/command`) no longer counts for the run it was sent
//!   into. The `Handled` result of a follow-up the harness handled
//!   ([`HookSession::follow_up_replied`]) takes nothing from the user's messages.
//! - Runs follow the event stream, not the order the shell gets to the events: right before a
//!   user message goes out the shell reports how many `Settled` events the stream delivered
//!   ([`HookSession::settles_seen`]), so a message sent after a settle the engine has not handled
//!   yet belongs to the run after that settle (and cancels the settled run's follow-up) even while
//!   earlier events wait behind slow commands, however many messages and settles are in flight.
//! - `sessionEnd` fires once, when the agent exits or the chat is stopped, with the paths the chat
//!   changed (the latest 256, within 16 KiB).
//! - A utility agent fires nothing.
//! - The Harness family hook "Save what you learned" does not fire while the chat's memory guards
//!   skip it, and asks about one kind only while the other may not be saved ([`MemoryGuardState`],
//!   set with [`HookSession::set_memory_guards`]).
//! - An eligible user prompt carries a short "Memory settings" part when the chat's memory guards
//!   changed since the agent was told last ([`SendPlan::memory_notice`]). A chat where everything
//!   may be saved starts with nothing to tell, and so does every new, switched, forked or cloned
//!   session.

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::compose::ResolvedPart;
use super::plan::{ToolEnv, matches_tool, planned_part, sort_parts};
use super::run::{RunState, SettleClock, TouchedLog, Waiting, commands_only, handled_without_run, is_eligible};
use crate::model::{AgentAction, AgentEvent, AgentReply, HookAction, HookEvent, PromptStatus};

pub use super::plan::{
    ChatContext, CommandRequest, EventOutcome, Firing, FollowUpPlan, HookSet, PlannedPart, SendPlan,
};

mod memory_gate;
mod memory_state;
mod user_marks;

pub use memory_gate::{MemorySaveGate, memory_save_part};
pub use memory_state::{MEMORY_NOTICE_ID, MEMORY_NOTICE_NAME, MemoryGuardState, SaveRule};

pub(crate) use memory_state::Announcement;

/// Who sends an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOrigin {
    /// The user, from the composer or a desktop command.
    User,
    /// The engine's own follow-up prompt.
    Hook,
}

/// The hook state of one chat. See the module docs.
#[derive(Debug)]
pub struct HookSession {
    chat: ChatContext,
    armed: bool,
    current: RunState,
    waiting: Option<Waiting>,
    next_run: u64,
    next_seq: u64,
    next_ticket: u64,
    pending_ticket: Option<u64>,
    chat_touched: TouchedLog,
    ended: bool,
    clock: SettleClock,
    /// Follow-ups the harness handled without a run whose `Handled` result is still to come.
    hook_handled: u32,
    /// Where the chat's memory guards stand now.
    memory: MemoryGuardState,
    /// Where they stood when the agent was last told.
    announced: MemoryGuardState,
}

impl HookSession {
    /// The state of a chat that is starting (`chat.fresh`: a new chat, `chat.utility`: a utility
    /// agent that never fires hooks).
    pub fn new(chat: ChatContext) -> Self {
        Self {
            armed: chat.fresh && !chat.utility,
            chat,
            current: RunState::new(0),
            waiting: None,
            next_run: 1,
            next_seq: 0,
            next_ticket: 0,
            pending_ticket: None,
            chat_touched: TouchedLog::default(),
            ended: false,
            clock: SettleClock::default(),
            hook_handled: 0,
            memory: MemoryGuardState::OPEN,
            announced: MemoryGuardState::OPEN,
        }
    }

    /// The chat.
    pub fn chat(&self) -> &ChatContext {
        &self.chat
    }

    /// True while the next eligible prompt fires `sessionStart`.
    pub fn is_armed(&self) -> bool {
        self.armed
    }

    /// The chat's stream has delivered `seen` `Settled` events so far, counted where the events
    /// arrive (before they wait for [`HookSession::on_event`]). Call it right before
    /// [`HookSession::sending`], so a message sent after a settle the engine has not handled yet
    /// belongs to the next run.
    pub fn settles_seen(&mut self, seen: u64) {
        self.clock.saw(seen);
    }

    /// The chat moved to `folder` (its project root `project`): later firings run there and see
    /// that project.
    pub fn move_to(&mut self, folder: PathBuf, project: Option<String>) {
        self.chat.folder = folder;
        self.chat.project = project;
    }

    /// Call before `action` is prepared. Returns the context to add to it (empty for anything but
    /// an eligible user prompt). A new, switched, forked or cloned session takes effect here; a
    /// user message counts only once [`HookSession::sending`] is called.
    pub fn on_send(&mut self, hooks: &HookSet, action: &AgentAction, origin: SendOrigin) -> SendPlan {
        if self.chat.utility {
            return SendPlan::default();
        }

        match action {
            AgentAction::NewSession => {
                self.armed = true;
                self.forget_run();
                self.forget_announced();
                return SendPlan::default();
            }
            AgentAction::SwitchSession { .. } | AgentAction::Fork { .. } | AgentAction::Clone => {
                self.armed = false;
                self.forget_run();
                self.forget_announced();
                return SendPlan::default();
            }
            _ => {}
        }

        let is_message = matches!(
            action,
            AgentAction::Prompt { .. } | AgentAction::Steer { .. } | AgentAction::FollowUp { .. }
        );

        if !is_message || origin == SendOrigin::Hook {
            return SendPlan::default();
        }

        let mut plan = SendPlan {
            user_message: true,
            ..SendPlan::default()
        };

        if !is_eligible(action) {
            return plan;
        }

        self.announce_memory(&mut plan);

        if self.armed {
            self.armed = false;
            plan.consumed_start = true;
            plan.firings
                .push(self.firing(hooks, HookEvent::SessionStart, ToolEnv::default()));
        }

        plan.firings
            .push(self.firing(hooks, HookEvent::PromptSubmit, ToolEnv::default()));
        plan.firings.retain(|firing| !firing.is_empty());
        plan
    }

    /// Call after the context of `plan` was composed: `resolved` is what its firings produced,
    /// `delivered` the ids of the hooks whose context the message carries (with
    /// [`MEMORY_NOTICE_ID`] when it carries the memory settings notice). When `sessionStart`
    /// produced context but none of it fit in the message, it fires on the next eligible prompt
    /// again. A `sessionStart` that produced nothing (a command without output) stays consumed. A
    /// notice that did not fit is told on the next eligible prompt.
    pub fn context_composed(&mut self, plan: &SendPlan, resolved: &[ResolvedPart], delivered: &[String]) {
        self.memory_announced(plan, delivered);

        if !plan.consumed_start {
            return;
        }

        let start_ids: BTreeSet<&str> = plan
            .firings
            .iter()
            .filter(|firing| firing.event == HookEvent::SessionStart)
            .flat_map(|firing| firing.parts.iter().map(PlannedPart::hook_id))
            .collect();

        let produced = resolved.iter().any(|part| start_ids.contains(part.hook_id.as_str()));
        let carried = delivered.iter().any(|id| start_ids.contains(id.as_str()));

        if produced && !carried {
            self.armed = true;
        }
    }

    /// Call for every event of the chat's stream.
    pub fn on_event(&mut self, hooks: &HookSet, event: &AgentEvent) -> EventOutcome {
        if self.chat.utility {
            return EventOutcome::Nothing;
        }

        match event {
            AgentEvent::ToolEnd {
                name,
                is_error,
                touched_paths,
                ..
            } => self.tool_ended(hooks, name, *is_error, touched_paths),
            AgentEvent::PromptResult { status, .. } => {
                match status {
                    PromptStatus::Error => self.current.failed = true,
                    PromptStatus::Aborted => self.current.aborted = true,
                    // The engine's own follow-up never counted, so its result takes nothing.
                    PromptStatus::Handled if self.hook_handled > 0 => self.hook_handled -= 1,
                    // A message the harness handled itself started no run: it does not count.
                    PromptStatus::Handled => {
                        self.current.user_messages = self.current.user_messages.saturating_sub(1);
                    }
                    PromptStatus::Completed => {}
                }

                EventOutcome::Nothing
            }
            AgentEvent::Settled { aborted } => self.settled(hooks, aborted.unwrap_or(false)),
            AgentEvent::Exited { .. } => self
                .on_stop(hooks)
                .map_or(EventOutcome::Nothing, EventOutcome::SessionEnd),
            _ => EventOutcome::Nothing,
        }
    }

    /// Reports what the `toolEnd` commands of `firing` produced (`outputs`: the parts of
    /// [`super::run_firing`]; a hook without one produced nothing). Returns the follow-up when the
    /// run already settled and this was the last report it waited for.
    pub fn on_tool_output(&mut self, firing: &Firing, outputs: &[ResolvedPart]) -> Option<FollowUpPlan> {
        let record = |run: &mut RunState| {
            for request in firing.commands() {
                let output = outputs.iter().find(|part| part.hook_id == request.hook_id).cloned();
                run.record_output(firing.seq, &request.hook_id, output);
            }

            run.outstanding = run.outstanding.saturating_sub(1);
        };

        if firing.run == self.current.id {
            record(&mut self.current);
            return None;
        }

        let waiting = self.waiting.as_mut().filter(|waiting| waiting.run.id == firing.run)?;
        record(&mut waiting.run);

        if waiting.run.outstanding > 0 {
            return None;
        }

        let waiting = self.waiting.take()?;
        self.follow_up(waiting.run, waiting.turn_parts, !waiting.cancelled)
    }

    /// True (once) when the follow-up `ticket` may still be sent: no user message went out since
    /// it was planned. Send it as a prompt from [`SendOrigin::Hook`].
    pub fn take_follow_up(&mut self, ticket: u64) -> bool {
        if self.pending_ticket == Some(ticket) {
            self.pending_ticket = None;
            return true;
        }

        false
    }

    /// Call with the agent's reply to the follow-up just sent (from [`SendOrigin::Hook`]), before
    /// the next event. When the harness handled it without a run, the next `Handled` result is the
    /// follow-up's: it leaves the user messages of the run alone.
    pub fn follow_up_replied(&mut self, reply: &AgentReply) {
        if handled_without_run(reply) {
            self.hook_handled += 1;
        }
    }

    /// The chat is stopped: the `sessionEnd` commands to run, once per chat (`Exited` after a
    /// stop fires nothing more). `None` when there are none.
    pub fn on_stop(&mut self, hooks: &HookSet) -> Option<Firing> {
        if self.chat.utility || self.ended {
            return None;
        }

        self.ended = true;

        let touched = self.chat_touched.paths();
        let firing = self.firing(
            hooks,
            HookEvent::SessionEnd,
            ToolEnv {
                tool: None,
                touched: &touched,
            },
        );

        let commands = commands_only(firing);

        (!commands.is_empty()).then_some(commands)
    }

    fn tool_ended(&mut self, hooks: &HookSet, name: &str, is_error: bool, touched: &[String]) -> EventOutcome {
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

    fn settled(&mut self, hooks: &HookSet, aborted: bool) -> EventOutcome {
        let (superseded, starting) = self.clock.settle();

        let mut next = RunState::new(self.next_run);
        next.user_messages = starting;
        self.next_run += 1;

        let mut run = std::mem::replace(&mut self.current, next);
        run.aborted |= aborted;

        // A run the engine's own prompt started fires nothing.
        if !run.user_sent() {
            return EventOutcome::Nothing;
        }

        let gated = memory_gate::gated(hooks, self.memory);
        let hooks = gated.as_ref().unwrap_or(hooks);

        let touched: Vec<String> = run.touched.iter().cloned().collect();
        let turn = self.firing(
            hooks,
            HookEvent::TurnEnd,
            ToolEnv {
                tool: None,
                touched: &touched,
            },
        );

        if superseded || !run.earned_follow_up() {
            let commands = commands_only(turn);
            return if commands.is_empty() {
                EventOutcome::Nothing
            } else {
                EventOutcome::TurnCommands(commands)
            };
        }

        if run.outstanding > 0 {
            self.waiting = Some(Waiting {
                run,
                turn_parts: turn.parts,
                cancelled: false,
            });
            return EventOutcome::Nothing;
        }

        self.follow_up(run, turn.parts, true)
            .map_or(EventOutcome::Nothing, EventOutcome::FollowUp)
    }

    /// The combined follow-up of a settled `run`: its `turnEnd` parts and its `toolEnd`
    /// contributions, in run order. Without `deliver` (a user message went out meanwhile) only
    /// the `turnEnd` commands are left, under a ticket that is never taken.
    fn follow_up(&mut self, run: RunState, mut parts: Vec<PlannedPart>, deliver: bool) -> Option<FollowUpPlan> {
        if deliver {
            parts.extend(run.tool_texts.into_iter().map(PlannedPart::Text));
            parts.extend(
                run.tool_outputs
                    .into_values()
                    .filter_map(|(_, output)| output.map(PlannedPart::Text)),
            );
        } else {
            parts.retain(|part| matches!(part, PlannedPart::Command(_)));
        }

        if parts.is_empty() {
            return None;
        }

        sort_parts(&mut parts);

        let ticket = self.next_ticket;
        self.next_ticket += 1;

        if deliver {
            self.pending_ticket = Some(ticket);
        }

        Some(FollowUpPlan {
            ticket,
            firing: Firing {
                event: HookEvent::TurnEnd,
                parts,
                run: run.id,
                seq: self.take_seq(),
            },
        })
    }

    /// The hooks of `event` as one firing of the current run.
    fn firing(&mut self, hooks: &HookSet, event: HookEvent, tool: ToolEnv<'_>) -> Firing {
        let parts = hooks
            .of_event(event)
            .map(|entry| planned_part(entry, &self.chat, event, tool))
            .collect();

        Firing {
            event,
            parts,
            run: self.current.id,
            seq: self.take_seq(),
        }
    }

    /// A new or other session in the same process: the run and any follow-up are dropped (a run
    /// still waiting for its `toolEnd` commands keeps only its `turnEnd` commands).
    fn forget_run(&mut self) {
        self.current = RunState::new(self.next_run);
        self.next_run += 1;
        self.pending_ticket = None;

        if let Some(waiting) = &mut self.waiting {
            waiting.cancelled = true;
        }
    }

    fn take_seq(&mut self) -> u64 {
        self.next_seq += 1;
        self.next_seq
    }
}
