//! How a user message counts for the runs of a [`HookSession`], and how a message that was never
//! delivered is taken back.

use super::HookSession;
use crate::hooks::plan::{SendPlan, UserMark};

impl HookSession {
    /// Call right before the message of `plan` is delivered, after [`HookSession::settles_seen`]
    /// with the latest count: a user message counts for the run it starts or joins (the run after
    /// the latest settle the stream delivered) and cancels a follow-up not sent yet. Does nothing
    /// for anything but a user message, or a second time.
    pub fn sending(&mut self, plan: &mut SendPlan) {
        if !plan.user_message || plan.previous.is_some() {
            return;
        }

        plan.previous = Some(UserMark {
            run: self.current.id,
            user_messages: self.current.user_messages,
            deferred: self.clock.deferred(),
            pending_ticket: self.pending_ticket,
            waiting_cancelled: self.waiting.as_ref().map(|waiting| waiting.cancelled),
        });

        if self.clock.user_message() {
            self.current.user_messages += 1;
        }

        if let Some(waiting) = &mut self.waiting {
            waiting.cancelled = true;
        }

        self.pending_ticket = None;
    }

    /// Call when the action of `plan` could not be delivered: `sessionStart` fires on the next
    /// eligible prompt again, the message does not count as sent and the follow-up it cancelled
    /// may go out again.
    pub fn send_failed(&mut self, plan: &SendPlan) {
        if plan.consumed_start {
            self.armed = true;
        }

        let Some(previous) = &plan.previous else {
            return;
        };

        if previous.run == self.current.id {
            self.current.user_messages = previous.user_messages;
        }

        self.clock.restore_deferred(previous.deferred.clone());
        self.pending_ticket = previous.pending_ticket;

        if let (Some(waiting), Some(cancelled)) = (&mut self.waiting, previous.waiting_cancelled) {
            waiting.cancelled = cancelled;
        }
    }
}
