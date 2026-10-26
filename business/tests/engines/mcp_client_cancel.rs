//! Cancelling a request the client stopped waiting for: the server hears `notifications/cancelled`
//! with the request's id, then the end of its input, and is killed only when it does not exit
//! within the grace period. The test plays the server over in-memory pipes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use harness_business::Result;
use harness_business::ecosystem::{McpClient, McpOptions};
use harness_business::process::{BoxFuture, ChildHandle, ExecSpec, ExitInfo, ProcessSpawner, SpawnedProcess};
use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};
use tokio::sync::Notify;

/// The child of the played server: it exits when the test says so, or when killed.
struct PlayedChild {
    exited: Arc<Notify>,
    killed: Arc<AtomicBool>,
}

impl ChildHandle for PlayedChild {
    fn id(&self) -> Option<u32> {
        None
    }

    fn wait(&mut self) -> BoxFuture<'_, Result<ExitInfo>> {
        Box::pin(async move {
            if !self.killed.load(Ordering::SeqCst) {
                self.exited.notified().await;
            }

            Ok(ExitInfo {
                code: Some(0),
                signal: None,
            })
        })
    }

    fn start_kill(&mut self) -> Result<()> {
        self.killed.store(true, Ordering::SeqCst);
        self.exited.notify_one();
        Ok(())
    }
}

/// Hands out one played server.
struct PlayedSpawner(Mutex<Option<SpawnedProcess>>);

impl ProcessSpawner for PlayedSpawner {
    fn spawn(&self, _spec: &ExecSpec) -> Result<SpawnedProcess> {
        Ok(self.0.lock().unwrap().take().expect("one server"))
    }
}

/// The test's side of the server: what the client writes, and where its answers go.
struct Server {
    input: BufReader<DuplexStream>,
    output: DuplexStream,
    exited: Arc<Notify>,
    killed: Arc<AtomicBool>,
}

impl Server {
    /// The next message the client sent, `None` at the end of the input.
    async fn next(&mut self) -> Option<Value> {
        let mut line = String::new();
        let read = self.input.read_line(&mut line).await.expect("read");

        (read > 0).then(|| serde_json::from_str(&line).expect("JSON"))
    }

    async fn answer(&mut self, id: &Value, result: Value) {
        let line = format!("{}\n", json!({ "jsonrpc": "2.0", "id": id, "result": result }));
        self.output.write_all(line.as_bytes()).await.expect("write");
    }
}

/// A client connected to a played server, the server past the handshake, and a `tools/call` the
/// server never answers sent and given up on.
async fn given_up_call() -> (McpClient, Server) {
    let (client_stdin, server_input) = tokio::io::duplex(64 * 1024);
    let (server_output, client_stdout) = tokio::io::duplex(64 * 1024);
    let (exited, killed) = (Arc::new(Notify::new()), Arc::new(AtomicBool::new(false)));

    let process = SpawnedProcess {
        stdin: Box::new(client_stdin),
        stdout: Box::new(client_stdout),
        stderr: Box::new(tokio::io::empty()),
        child: Box::new(PlayedChild {
            exited: exited.clone(),
            killed: killed.clone(),
        }),
    };
    let spawner = PlayedSpawner(Mutex::new(Some(process)));
    let mut server = Server {
        input: BufReader::new(server_input),
        output: server_output,
        exited,
        killed,
    };

    let spec = ExecSpec::new("played");
    let handshake = async {
        let initialize = server.next().await.expect("initialize");
        let result = json!({ "protocolVersion": "2025-11-25", "serverInfo": { "name": "harness-memory" } });
        server.answer(&initialize["id"], result).await;
        server.next().await.expect("initialized");
    };
    let (client, ()) = tokio::join!(McpClient::connect(&spawner, &spec, McpOptions::default()), handshake);
    let mut client = client.expect("connected");

    let gave_up = tokio::time::timeout(Duration::from_millis(100), client.call_tool("script_run", Map::new())).await;
    assert!(gave_up.is_err(), "the played server never answers");

    let call = server.next().await.expect("tools/call");
    assert_eq!(call["method"], "tools/call");

    (client, server)
}

#[tokio::test]
async fn a_cancel_names_the_request_then_closes_the_input_of_a_server_that_exits() {
    let (client, mut server) = given_up_call().await;

    let heard = async {
        let cancel = server.next().await.expect("the cancel");
        let end = server.next().await;
        server.exited.notify_one();
        (cancel, end)
    };

    let started = Instant::now();
    let ((), (cancel, end)) = tokio::join!(client.cancel(), heard);

    assert_eq!(cancel["method"], "notifications/cancelled");
    assert_eq!(cancel["params"]["requestId"], 2, "the tools/call after the handshake");
    assert_eq!(end, None, "the input ends after the cancel");
    assert!(
        !server.killed.load(Ordering::SeqCst),
        "a server that exits is not killed"
    );
    assert!(started.elapsed() < Duration::from_secs(1), "{:?}", started.elapsed());
}

#[tokio::test]
async fn a_server_that_stays_after_the_cancel_is_killed() {
    let (client, mut server) = given_up_call().await;

    let heard = async {
        server.next().await.expect("the cancel");
        server.next().await
    };

    let ((), end) = tokio::join!(client.cancel(), heard);

    assert_eq!(end, None);
    assert!(
        server.killed.load(Ordering::SeqCst),
        "the server was killed after the grace period"
    );
}
