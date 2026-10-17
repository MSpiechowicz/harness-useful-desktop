//! The MCP client and the pooled dashboard connection against scripted servers
//! (`fake_agent --dialect mcp`, scenarios `tests/fixtures/streams/mcp_*.jsonl`).

mod agent_support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_support::scenario;
use harness_business::BusinessError;
use harness_business::ecosystem::{Dashboard, DashboardOptions, McpClient, McpOptions};
use harness_business::env::Env;
use harness_business::model::{DashboardAvailability, DashboardQuery, DashboardTool};
use harness_business::process::{ExecSpec, ProcessSpawner, SpawnedProcess, TokioSpawner};
use serde_json::{Map, Value, json};
use tempfile::TempDir;

fn fake_server(name: &str) -> ExecSpec {
    ExecSpec::new(env!("CARGO_BIN_EXE_fake_agent")).args([
        "--dialect".to_owned(),
        "mcp".to_owned(),
        "--scenario".to_owned(),
        scenario(name).to_string_lossy().into_owned(),
    ])
}

fn quick() -> McpOptions {
    McpOptions {
        request_timeout: Duration::from_millis(500),
        ..McpOptions::default()
    }
}

async fn connect(name: &str) -> McpClient {
    McpClient::connect(&TokioSpawner, &fake_server(name), quick())
        .await
        .unwrap_or_else(|error| panic!("{name} connects: {error}"))
}

fn arguments(value: Value) -> Map<String, Value> {
    value.as_object().cloned().expect("an object")
}

#[tokio::test]
async fn handshake_then_lists_tools_and_calls_one() {
    let mut client = connect("mcp_tools.jsonl").await;
    assert_eq!(client.server_name(), Some("harness-dashboard"));

    let tools = client.list_tools().await.expect("tools/list");
    assert_eq!(tools.len(), 8);
    assert!(tools.iter().any(|name| name == "budget_status"));

    let output = client.call_tool("limits", Map::new()).await.expect("tools/call");
    assert_eq!(output.structured, Some(json!({ "windows": [] })));
    assert_eq!(output.text.as_deref(), Some("{\"windows\":[]}"));
    assert!(!output.is_error);
    assert!(client.is_usable());

    client.shutdown().await;
}

#[tokio::test]
async fn answers_are_matched_by_id_past_notifications_and_stale_answers() {
    let mut client = connect("mcp_call.jsonl").await;

    let summary = client
        .call_tool("usage_summary", arguments(json!({ "range": "7d", "provider": "omp" })))
        .await
        .expect("usage_summary");
    assert_eq!(summary.structured, Some(json!({ "range": "7d", "costUsd": 1.5 })));

    let session = client
        .call_tool("session_cost", arguments(json!({ "session_id": "0199c1f2-unknown" })))
        .await
        .expect("session_cost");
    assert!(session.is_error);
    assert_eq!(session.structured, None);
    assert_eq!(
        session.text.as_deref(),
        Some("No session 0199c1f2-unknown in the database yet.")
    );

    client.shutdown().await;
}

#[tokio::test]
async fn a_json_rpc_error_fails_only_its_request_and_is_redacted() {
    let mut client = connect("mcp_error.jsonl").await;

    let error = client.call_tool("nope", Map::new()).await.expect_err("an RPC error");
    let message = error.to_string();
    assert!(matches!(error, BusinessError::Protocol(_)), "{error:?}");
    assert!(
        message.contains("tools/call failed (-32602): Unknown tool: nope"),
        "{message}"
    );
    assert!(!message.contains("abc123secret"), "{message}");
    assert!(client.is_usable());

    let next = client.call_tool("limits", Map::new()).await.expect("the next call");
    assert_eq!(next.text.as_deref(), Some("[1,2]"));
    assert_eq!(next.structured, None);

    client.shutdown().await;
}

#[tokio::test]
async fn a_silent_server_times_out_and_the_client_is_retired() {
    let mut client = connect("mcp_timeout.jsonl").await;

    let error = client.call_tool("limits", Map::new()).await.expect_err("a timeout");
    assert!(matches!(error, BusinessError::Timeout(_)), "{error:?}");
    assert!(!client.is_usable());

    let refused = client.list_tools().await.expect_err("unusable");
    assert!(matches!(refused, BusinessError::Process(_)), "{refused:?}");

    // The scenario ignores end of input: shutdown has to kill it.
    tokio::time::timeout(Duration::from_secs(10), client.shutdown())
        .await
        .expect("shutdown kills a hung server");
}

#[tokio::test]
async fn an_oversized_answer_fails_its_request_and_the_stream_stays_in_sync() {
    let mut client = connect("mcp_oversized.jsonl").await;

    let error = client.call_tool("breakdown", Map::new()).await.expect_err("too large");
    assert!(matches!(error, BusinessError::Protocol(_)), "{error:?}");
    assert!(error.to_string().contains("could not be read"), "{error}");
    assert!(client.is_usable());

    let next = client.call_tool("limits", Map::new()).await.expect("next answer");
    assert_eq!(next.structured, Some(json!({ "ok": true })));

    client.shutdown().await;
}

#[tokio::test]
async fn a_server_that_exits_mid_call_retires_the_client() {
    let mut client = connect("mcp_exit.jsonl").await;

    let error = client.call_tool("limits", Map::new()).await.expect_err("server gone");
    assert!(matches!(error, BusinessError::Process(_)), "{error:?}");
    assert!(!client.is_usable());
}

#[tokio::test]
async fn a_rejected_handshake_fails_to_connect() {
    let error = McpClient::connect(&TokioSpawner, &fake_server("mcp_init_error.jsonl"), quick())
        .await
        .expect_err("handshake rejected");

    assert!(error.to_string().contains("initialize failed (-32602)"), "{error}");
}

/// Hands out one server process that has already exited, so the handshake's write fails before
/// any read.
struct ExitedServer(Mutex<Option<SpawnedProcess>>);

impl ProcessSpawner for ExitedServer {
    fn spawn(&self, _spec: &ExecSpec) -> harness_business::Result<SpawnedProcess> {
        Ok(self.0.lock().unwrap().take().expect("spawned once"))
    }
}

#[tokio::test]
async fn a_server_that_exited_before_the_handshake_is_reported_as_such() {
    // `fake_agent` without a dialect exits at once with its usage.
    let spec = ExecSpec::new(env!("CARGO_BIN_EXE_fake_agent"));
    let mut process = TokioSpawner.spawn(&spec).expect("fake_agent starts");
    process.child.wait().await.expect("fake_agent exits");
    let spawner = ExitedServer(Mutex::new(Some(process)));

    let error = McpClient::connect(&spawner, &spec, quick())
        .await
        .expect_err("nothing answers");

    assert!(matches!(error, BusinessError::Process(_)), "{error:?}");
    assert!(
        error.to_string().contains("exited before answering initialize"),
        "{error}"
    );
}

/// Starts `fake_agent` with a scenario for every spawn and records the requested specs.
struct ServerSpawner {
    scenario: &'static str,
    requested: Mutex<Vec<ExecSpec>>,
}

impl ServerSpawner {
    fn new(scenario: &'static str) -> Arc<Self> {
        Arc::new(Self {
            scenario,
            requested: Mutex::new(Vec::new()),
        })
    }

    fn requested(&self) -> Vec<ExecSpec> {
        self.requested.lock().unwrap().clone()
    }
}

impl ProcessSpawner for ServerSpawner {
    fn spawn(&self, spec: &ExecSpec) -> harness_business::Result<SpawnedProcess> {
        self.requested.lock().unwrap().push(spec.clone());
        TokioSpawner.spawn(&fake_server(self.scenario))
    }
}

/// A home with an executable `~/.local/bin/harness-dashboard` (`.exe` on Windows; never run: the spawner replaces it).
fn home_with_dashboard() -> (TempDir, Env, PathBuf) {
    let home = tempfile::tempdir().expect("temp home");
    let bin = home.path().join(".local/bin");
    std::fs::create_dir_all(&bin).expect("create bin");

    let binary = bin.join(format!("harness-dashboard{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&binary, "").expect("write placeholder");
    make_executable(&binary);

    let env = Env::for_tests(home.path());
    (home, env, binary)
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

fn pool(spawner: Arc<ServerSpawner>, idle: Duration) -> Dashboard {
    Dashboard::new(
        spawner,
        DashboardOptions {
            idle_timeout: idle,
            client: quick(),
        },
    )
}

fn query(tool: DashboardTool, args: Value) -> DashboardQuery {
    DashboardQuery { tool, args }
}

#[tokio::test]
async fn availability_and_queries_share_one_server_run_in_the_home_directory() {
    let (_home, env, binary) = home_with_dashboard();
    let spawner = ServerSpawner::new("mcp_tools.jsonl");
    let dashboard = pool(spawner.clone(), Duration::from_secs(30));

    assert_eq!(dashboard.availability(&env).await, DashboardAvailability::Available);

    let limits = dashboard
        .query(&env, &query(DashboardTool::Limits, Value::Null))
        .await
        .expect("limits");
    assert_eq!(limits.tool, DashboardTool::Limits);
    assert_eq!(limits.data, json!({ "windows": [] }));
    assert_eq!(limits.text, None);
    assert!(!limits.is_error);

    let requested = spawner.requested();
    assert_eq!(requested.len(), 1, "one server for both");
    assert_eq!(requested[0].program, binary);
    assert_eq!(requested[0].args, ["mcp"], "never --db");
    assert_eq!(requested[0].cwd.as_deref(), Some(env.home.as_path()));

    dashboard.shutdown().await;
}

#[tokio::test]
async fn the_server_stops_when_idle_and_starts_again_on_the_next_query() {
    let (_home, env, _binary) = home_with_dashboard();
    let spawner = ServerSpawner::new("mcp_tools.jsonl");
    let dashboard = pool(spawner.clone(), Duration::from_millis(200));

    assert_eq!(dashboard.availability(&env).await, DashboardAvailability::Available);
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert_eq!(dashboard.availability(&env).await, DashboardAvailability::Available);

    assert_eq!(spawner.requested().len(), 2);
    dashboard.shutdown().await;
}

#[tokio::test]
async fn tool_errors_come_back_as_results_with_their_text() {
    let (_home, env, _binary) = home_with_dashboard();
    let dashboard = pool(ServerSpawner::new("mcp_call.jsonl"), Duration::from_secs(30));

    let summary = dashboard
        .query(
            &env,
            &query(DashboardTool::UsageSummary, json!({ "range": "7d", "provider": "omp" })),
        )
        .await
        .expect("usage_summary");
    assert_eq!(summary.data, json!({ "range": "7d", "costUsd": 1.5 }));

    let session = dashboard
        .query(
            &env,
            &query(DashboardTool::SessionCost, json!({ "session_id": "0199c1f2-unknown" })),
        )
        .await
        .expect("session_cost");
    assert!(session.is_error);
    assert_eq!(session.data, Value::Null);
    assert!(session.text.unwrap().starts_with("No session"));

    dashboard.shutdown().await;
}

#[tokio::test]
async fn tool_error_text_is_redacted_and_clipped_before_it_reaches_the_ui() {
    let (_home, env, _binary) = home_with_dashboard();
    let dashboard = pool(
        ServerSpawner::new("mcp_tool_error_secret.jsonl"),
        Duration::from_secs(30),
    );

    let limits = dashboard
        .query(&env, &query(DashboardTool::Limits, Value::Null))
        .await
        .expect("limits");
    let text = limits.text.expect("an error text");

    assert!(limits.is_error);
    assert!(text.starts_with("Database error password="), "{text}");
    assert!(!text.contains("hunter2plain"), "{text}");
    assert_eq!(text.chars().count(), 300, "{text}");

    dashboard.shutdown().await;
}

#[tokio::test]
async fn invalid_arguments_are_refused_before_any_server_starts() {
    let (_home, env, _binary) = home_with_dashboard();
    let spawner = ServerSpawner::new("mcp_tools.jsonl");
    let dashboard = pool(spawner.clone(), Duration::from_secs(30));

    let error = dashboard
        .query(
            &env,
            &query(
                DashboardTool::Breakdown,
                json!({ "dimension": "model", "db": "/tmp/x.db" }),
            ),
        )
        .await
        .expect_err("unknown field");

    assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
    assert!(spawner.requested().is_empty());
}

#[tokio::test]
async fn a_missing_binary_is_not_installed() {
    let home = tempfile::tempdir().expect("temp home");
    let env = Env::for_tests(home.path());
    let spawner = ServerSpawner::new("mcp_tools.jsonl");
    let dashboard = pool(spawner.clone(), Duration::from_secs(30));

    assert_eq!(dashboard.availability(&env).await, DashboardAvailability::NotInstalled);

    let error = dashboard
        .query(&env, &query(DashboardTool::Limits, json!({})))
        .await
        .expect_err("not installed");
    assert!(matches!(error, BusinessError::NotFound(_)), "{error:?}");
    assert!(spawner.requested().is_empty());
}

#[tokio::test]
async fn an_old_dashboard_or_a_broken_server_is_reported_as_failed() {
    let (_home, env, _binary) = home_with_dashboard();

    let old = pool(ServerSpawner::new("mcp_tools_old.jsonl"), Duration::from_secs(30));
    let DashboardAvailability::Failed { message } = old.availability(&env).await else {
        panic!("an old dashboard is not available");
    };
    assert!(message.contains("failures, tips"), "{message}");
    old.shutdown().await;

    let broken = pool(ServerSpawner::new("mcp_init_error.jsonl"), Duration::from_secs(30));
    let DashboardAvailability::Failed { message } = broken.availability(&env).await else {
        panic!("a broken server is not available");
    };
    assert!(message.contains("initialize failed"), "{message}");
}
