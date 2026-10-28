//! Requests waiting for their `response` frame, correlated by `id` only.
//!
//! Responses may arrive in any order; a response with an unknown id is ignored. When the agent
//! closes its output every waiting request fails with a process error.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::oneshot;

use crate::error::{BusinessError, Result};
use crate::model::AgentAction;

/// Time limit for a prompt to be admitted (its acknowledgement, not its completion).
pub const PROMPT_ACK_TIMEOUT: Duration = Duration::from_secs(30);

/// Time limit for `abort`.
pub const ABORT_TIMEOUT: Duration = Duration::from_secs(15);

/// Time limit for `compact`, which waits for a summary from the model.
pub const COMPACT_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Time limit for every other command.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// How long the agent may take to answer `action`.
pub fn timeout_for(action: &AgentAction) -> Duration {
    match action {
        AgentAction::Prompt { .. } => PROMPT_ACK_TIMEOUT,
        AgentAction::Abort => ABORT_TIMEOUT,
        AgentAction::Compact { .. } => COMPACT_TIMEOUT,
        _ => DEFAULT_TIMEOUT,
    }
}

/// A `response` frame.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcResponse {
    /// The request id; `None` for failures the agent could not correlate.
    pub id: Option<String>,
    /// The command the response answers.
    pub command: String,
    /// True when the command succeeded.
    pub success: bool,
    /// Command result.
    pub data: Option<Value>,
    /// Failure message.
    pub error: Option<String>,
    /// Machine-readable failure reason when the agent names one (OMP: `session_busy`, `stale_cursor`).
    pub code: Option<String>,
}

/// What a waiting request was, so the reader can react to its response in stream order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    /// A `prompt` (its acknowledgement can complete the prompt, see `RpcDialect::on_prompt_ack`).
    Prompt,
    /// Anything else.
    Other,
}

type Reply = oneshot::Sender<Result<RpcResponse>>;

#[derive(Default)]
struct State {
    waiting: HashMap<String, (RequestKind, Reply)>,
    closed: Option<String>,
}

/// The table of waiting requests of one session.
#[derive(Default)]
pub struct PendingRequests {
    state: Mutex<State>,
}

impl PendingRequests {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers request `id`; fails once the session closed.
    pub fn register(&self, id: &str, kind: RequestKind) -> Result<oneshot::Receiver<Result<RpcResponse>>> {
        let mut state = self.lock();

        if let Some(reason) = &state.closed {
            return Err(BusinessError::Process(reason.clone()));
        }

        let (sender, receiver) = oneshot::channel();
        state.waiting.insert(id.to_owned(), (kind, sender));

        Ok(receiver)
    }

    /// The kind of waiting request `id`.
    pub fn kind(&self, id: &str) -> Option<RequestKind> {
        self.lock().waiting.get(id).map(|(kind, _)| *kind)
    }

    /// Completes the request the response belongs to. Returns false for unknown ids.
    pub fn resolve(&self, response: RpcResponse) -> bool {
        let Some(id) = response.id.clone() else {
            return false;
        };

        match self.lock().waiting.remove(&id) {
            Some((_, reply)) => {
                reply.send(Ok(response)).ok();
                true
            }
            None => false,
        }
    }

    /// Fails request `id` (if still waiting).
    pub fn fail(&self, id: &str, error: BusinessError) {
        if let Some((_, reply)) = self.lock().waiting.remove(id) {
            reply.send(Err(error)).ok();
        }
    }

    /// Forgets request `id` without answering it (after a timeout).
    pub fn forget(&self, id: &str) {
        self.lock().waiting.remove(id);
    }

    /// Fails every waiting request with a process error and refuses new ones.
    pub fn close(&self, reason: &str) {
        let waiting = {
            let mut state = self.lock();
            state.closed.get_or_insert_with(|| reason.to_owned());
            std::mem::take(&mut state.waiting)
        };

        for (_, (_, reply)) in waiting {
            reply.send(Err(BusinessError::Process(reason.to_owned()))).ok();
        }
    }

    /// Number of waiting requests.
    pub fn len(&self) -> usize {
        self.lock().waiting.len()
    }

    /// True when nothing is waiting.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Waits for `receiver` at most `limit`, extending the deadline while `keep_waiting` holds (the
/// agent has a dialog open, so the user is the one being waited for). `label` names the command.
pub async fn await_response(
    receiver: oneshot::Receiver<Result<RpcResponse>>,
    limit: Duration,
    label: &str,
    keep_waiting: impl Fn() -> bool,
) -> Result<RpcResponse> {
    let mut receiver = receiver;

    loop {
        match tokio::time::timeout(limit, &mut receiver).await {
            Ok(Ok(result)) => return result,
            Ok(Err(_)) => return Err(BusinessError::Process("agent session closed".to_owned())),
            Err(_) if keep_waiting() => continue,
            Err(_) => {
                return Err(BusinessError::Timeout(format!(
                    "{label} got no answer within {} s",
                    limit.as_secs()
                )));
            }
        }
    }
}
