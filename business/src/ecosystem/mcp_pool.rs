//! One pooled [`McpClient`] for a companion's `mcp` server, shared by the dashboard
//! ([`super::dashboard`]) and the memory scripts ([`super::memory_scripts`]).
//!
//! [`McpPool`] keeps the client behind a mutex and reuses it while the requested [`ExecSpec`]
//! stays the same; it shuts the server down after [`PoolOptions::idle_timeout`] without use, and
//! starts it again on the next request. A request on a reused client that turns out to have exited
//! is sent once more on a fresh server.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Value};
use tokio::sync::Mutex;

use super::mcp_client::{McpClient, McpOptions, McpTool, ToolOutput};
use crate::error::{BusinessError, Result};
use crate::process::{ExecSpec, ProcessSpawner};

/// How a pooled client behaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolOptions {
    /// The server stops after this long without a request (30 s).
    pub idle_timeout: Duration,
    /// Request timeout and response cap of the client.
    pub client: McpOptions,
}

impl Default for PoolOptions {
    fn default() -> Self {
        Self {
            idle_timeout: Duration::from_secs(30),
            client: McpOptions::default(),
        }
    }
}

/// The pooled connection. See the module docs.
pub struct McpPool {
    spawner: Arc<dyn ProcessSpawner>,
    options: PoolOptions,
    slot: Arc<Mutex<Slot>>,
}

/// The running client, the spec it was started from and a counter of uses (for the idle timer).
#[derive(Default)]
struct Slot {
    client: Option<(ExecSpec, McpClient)>,
    uses: u64,
}

/// What [`McpPool::exec`] sends.
enum Request<'a> {
    ListTools,
    CallTool {
        name: &'a str,
        arguments: Map<String, Value>,
    },
}

/// What [`McpPool::exec`] gets back.
enum Reply {
    Tools(Vec<McpTool>),
    Tool(ToolOutput),
}

impl McpPool {
    /// A pool that starts servers through `spawner`.
    pub fn new(spawner: Arc<dyn ProcessSpawner>, options: PoolOptions) -> Self {
        Self {
            spawner,
            options,
            slot: Arc::new(Mutex::new(Slot::default())),
        }
    }

    /// The names of the tools of the server `spec` starts (`tools/list`).
    pub async fn list_tools(&self, spec: &ExecSpec) -> Result<Vec<String>> {
        Ok(self.tools(spec).await?.into_iter().map(|tool| tool.name).collect())
    }

    /// The tools of the server `spec` starts, with their arguments (`tools/list`).
    pub async fn tools(&self, spec: &ExecSpec) -> Result<Vec<McpTool>> {
        match self.exec(spec, &Request::ListTools).await? {
            Reply::Tools(tools) => Ok(tools),
            Reply::Tool(_) => Err(unexpected()),
        }
    }

    /// Calls tool `name` on the server `spec` starts (`tools/call`).
    pub async fn call_tool(&self, spec: &ExecSpec, name: &str, arguments: Map<String, Value>) -> Result<ToolOutput> {
        match self.exec(spec, &Request::CallTool { name, arguments }).await? {
            Reply::Tool(output) => Ok(output),
            Reply::Tools(_) => Err(unexpected()),
        }
    }

    /// Stops the server now (app shutdown, tests).
    pub async fn shutdown(&self) {
        let taken = self.slot.lock().await.client.take();

        if let Some((_, client)) = taken {
            client.shutdown().await;
        }
    }

    /// Sends `request` on the pooled client for `spec`, starting the server when there is none
    /// (or it was started from another spec, or it stopped).
    async fn exec(&self, spec: &ExecSpec, request: &Request<'_>) -> Result<Reply> {
        let mut slot = self.slot.lock().await;

        let reusable = matches!(&slot.client, Some((running, client)) if running == spec && client.is_usable());
        if !reusable && let Some((_, stale)) = slot.client.take() {
            tokio::spawn(stale.shutdown());
        }

        let mut outcome = self.exec_once(&mut slot, spec, request).await;

        if reusable && matches!(outcome, Err(BusinessError::Process(_))) {
            outcome = self.exec_once(&mut slot, spec, request).await;
        }

        slot.uses += 1;
        self.schedule_idle_shutdown(slot.uses);

        outcome
    }

    async fn exec_once(&self, slot: &mut Slot, spec: &ExecSpec, request: &Request<'_>) -> Result<Reply> {
        if slot.client.as_ref().is_none_or(|(_, client)| !client.is_usable()) {
            let started = McpClient::connect(self.spawner.as_ref(), spec, self.options.client.clone()).await?;
            slot.client = Some((spec.clone(), started));
        }

        let Some((_, client)) = slot.client.as_mut() else {
            return Err(BusinessError::Internal("the pooled MCP client is missing".to_owned()));
        };

        let outcome = match request {
            Request::ListTools => client.tools().await.map(Reply::Tools),
            Request::CallTool { name, arguments } => client.call_tool(name, arguments.clone()).await.map(Reply::Tool),
        };

        if !client.is_usable()
            && let Some((_, broken)) = slot.client.take()
        {
            tokio::spawn(broken.shutdown());
        }

        outcome
    }

    /// Shuts the server down after the idle timeout unless it was used again meanwhile.
    fn schedule_idle_shutdown(&self, uses: u64) {
        let slot = Arc::downgrade(&self.slot);
        let idle = self.options.idle_timeout;

        tokio::spawn(async move {
            tokio::time::sleep(idle).await;

            let Some(slot) = slot.upgrade() else {
                return;
            };

            let mut slot = slot.lock().await;
            if slot.uses != uses {
                return;
            }

            if let Some((_, client)) = slot.client.take() {
                drop(slot);
                client.shutdown().await;
            }
        });
    }
}

fn unexpected() -> BusinessError {
    BusinessError::Internal("unexpected MCP reply".to_owned())
}
