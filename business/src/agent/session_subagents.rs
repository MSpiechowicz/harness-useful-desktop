//! Stopping an RPC session's subagents (OMP `cancel_subagent`).
//!
//! OMP runs a `task` subagent in the background, so aborting the main turn leaves it running. The
//! reader follows which subagents are pending or running (`Shared::live_subagents`), and an abort
//! stops all of them at once after the turn's own abort was answered.

use std::future::Future;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;

use super::session::RpcSession;
use crate::error::Result;
use crate::model::{AgentAction, AgentReply};
use crate::pi_family::rpc::{PendingRequests, RequestKind};

/// Time limit for one `cancel_subagent`, so a stuck subagent cannot hold up the abort for long.
const STOP_SUBAGENT_TIMEOUT: Duration = Duration::from_secs(5);

/// Time limit for all the stops an abort sends together (a dialog the agent has open would otherwise hold them without end).
const STOP_SUBAGENTS_DEADLINE: Duration = Duration::from_secs(5);

impl RpcSession {
    /// Stops subagent `id`. OMP answers `cancelled: false` for one it does not run (finished,
    /// already stopped or unknown), which is still an `Ack`: the subagent is not running. A
    /// harness without the command refuses it, and that comes back as the error.
    pub(super) async fn stop_subagent(&self, id: &str) -> Result<AgentReply> {
        let action = AgentAction::StopSubagent { id: id.to_owned() };

        let request_id = self.next_id();
        let sessions_dir = self.sessions_dir();
        let context = crate::pi_family::rpc::EncodeContext {
            sessions_dir: &sessions_dir,
        };
        let frame = self.dialect.encode(&action, &request_id, &context)?;

        let _forget = ForgetOnDrop {
            pending: &self.shared.pending,
            id: &request_id,
        };

        self.request(&frame, &request_id, RequestKind::Other, STOP_SUBAGENT_TIMEOUT)
            .await?;

        Ok(AgentReply::Ack)
    }

    /// Stops every subagent still pending or running, all at once, within [`STOP_SUBAGENTS_DEADLINE`] in
    /// total. A refusal or a stop that did not finish in time is logged: it must not fail the abort that was
    /// already answered.
    pub(super) async fn stop_live_subagents(&self) {
        let ids = self.shared.live_subagents();
        if ids.is_empty() {
            return;
        }

        let stops = ids.iter().map(|id| async move {
            if let Err(error) = self.stop_subagent(id).await {
                tracing::debug!(%error, subagent = %id, "a subagent was not stopped with the aborted turn");
            }
        });

        if tokio::time::timeout(STOP_SUBAGENTS_DEADLINE, join_all(stops))
            .await
            .is_err()
        {
            tracing::debug!("some subagents were not stopped within the time limit of the aborted turn");
        }
    }
}

/// Runs `futures` side by side on the current task until all are done, polling them in order.
async fn join_all<F: Future<Output = ()>>(futures: impl IntoIterator<Item = F>) {
    let mut running: Vec<Pin<Box<F>>> = futures.into_iter().map(Box::pin).collect();

    std::future::poll_fn(|context| {
        running.retain_mut(|future| future.as_mut().poll(context).is_pending());

        if running.is_empty() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
}

/// Forgets a request that was dropped before its answer came (the abort's deadline passed), so it is not left waiting.
struct ForgetOnDrop<'a> {
    pending: &'a PendingRequests,
    id: &'a str,
}

impl Drop for ForgetOnDrop<'_> {
    fn drop(&mut self) {
        self.pending.forget(self.id);
    }
}
