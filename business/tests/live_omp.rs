//! Live smoke test against the installed `omp` (ignored by default).
//!
//! Run with `HARNESS_LIVE_OMP=1 cargo test -p harness_business --test live_omp -- --ignored`.
//! `omp --mode rpc` runs with `HOME` pointed at a temporary directory and the XDG data/config/
//! state/cache variables, `PI_CODING_AGENT_DIR` and the other OMP location variables removed, in
//! a temporary project, so the real `~/.omp` is never read or written. OMP refuses to start
//! without a model, so the temporary home gets a `models.yml` with one model of an offline
//! provider (`127.0.0.1:9`, nothing listens there) and no credentials. No prompt is ever sent:
//! the test only checks the handshake, `get_state`, the live command list and shutdown.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use harness_business::agent::{AgentDeps, AgentManager, CollectingSink};
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harnesses::Omp;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, AgentStartRequest, CommandSource, HarnessId};
use harness_business::pi_family::rpc::{EncodeContext, Frame, FrameReader, encode_frame};
use harness_business::process::{
    BoxFuture, ChildHandle, ExecSpec, ExitInfo, ProcessSpawner, SpawnedProcess, resolve_binary,
};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;

/// Variables that could point OMP at the real configuration.
const REMOVED_VARS: &[&str] = &[
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_STATE_HOME",
    "XDG_CACHE_HOME",
    "PI_CODING_AGENT_DIR",
    "PI_CODING_AGENT_SESSION_DIR",
    "PI_CONFIG_DIR",
    "OMP_PROFILE",
    "PI_PROFILE",
];

/// One model of an unreachable local provider: enough for OMP to start, never contacted.
const OFFLINE_MODELS: &str = "providers:
  offline-smoke:
    baseUrl: http://127.0.0.1:9/v1
    api: openai-completions
    auth: none
    models:
      - id: smoke-model
";

/// Spawns with an isolated home.
struct IsolatedSpawner {
    home: PathBuf,
}

struct LiveChild(tokio::process::Child);

impl ChildHandle for LiveChild {
    fn id(&self) -> Option<u32> {
        self.0.id()
    }

    fn wait(&mut self) -> BoxFuture<'_, harness_business::Result<ExitInfo>> {
        Box::pin(async move {
            let status = self
                .0
                .wait()
                .await
                .map_err(|error| harness_business::BusinessError::Process(error.to_string()))?;
            Ok(ExitInfo::from(status))
        })
    }

    fn start_kill(&mut self) -> harness_business::Result<()> {
        self.0.start_kill().ok();
        Ok(())
    }
}

impl ProcessSpawner for IsolatedSpawner {
    fn spawn(&self, spec: &ExecSpec) -> harness_business::Result<SpawnedProcess> {
        let mut command = tokio::process::Command::new(&spec.program);
        command
            .args(&spec.args)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        for name in REMOVED_VARS {
            command.env_remove(name);
        }

        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }

        let mut child = command
            .spawn()
            .map_err(|error| harness_business::BusinessError::Process(error.to_string()))?;

        Ok(SpawnedProcess {
            stdin: Box::new(child.stdin.take().expect("stdin")),
            stdout: Box::new(child.stdout.take().expect("stdout")),
            stderr: Box::new(child.stderr.take().expect("stderr")),
            child: Box::new(LiveChild(child)),
        })
    }
}

fn enabled() -> bool {
    std::env::var("HARNESS_LIVE_OMP").as_deref() == Ok("1")
}

/// Reads frames until one satisfies `predicate` (at most 20 s).
async fn read_until<R>(frames: &mut FrameReader<R>, predicate: impl Fn(&Value) -> bool) -> Value
where
    R: tokio::io::AsyncRead + Unpin,
{
    let search = async {
        loop {
            match frames.next_frame().await.expect("omp keeps stdout open") {
                Frame::Json(value) if predicate(&value) => return value,
                Frame::Json(_) => {}
                Frame::Fault(fault) => panic!("frame fault: {fault:?}"),
            }
        }
    };

    tokio::time::timeout(Duration::from_secs(20), search)
        .await
        .expect("omp answered in time")
}

fn response_to(id: &'static str) -> impl Fn(&Value) -> bool {
    move |frame| frame["type"] == "response" && frame["id"] == id
}

/// The raw protocol: ready, event filter, state and the live command list.
async fn raw_exchange(spawner: &IsolatedSpawner, spec: &ExecSpec) {
    let mut process = spawner.spawn(spec).expect("omp starts");
    let mut frames = FrameReader::new(process.stdout);

    let ready = read_until(&mut frames, |frame| frame["type"] == "ready").await;
    println!("ready: {ready}");
    assert_eq!(ready["protocolVersion"], 1);

    let context = EncodeContext {
        sessions_dir: Path::new("/nonexistent"),
    };
    let mut filter = Omp.dialect().after_handshake().remove(0);
    filter["id"] = json!("f-1");
    let state = Omp.dialect().encode(&AgentAction::GetState, "s-1", &context).unwrap();
    let commands = Omp
        .dialect()
        .encode(&AgentAction::RefreshCommands, "c-1", &context)
        .unwrap();

    for frame in [&filter, &state, &commands] {
        process.stdin.write_all(&encode_frame(frame)).await.unwrap();
    }
    process.stdin.flush().await.unwrap();

    let filtered = read_until(&mut frames, response_to("f-1")).await;
    assert_eq!(filtered["success"], true, "{filtered}");
    let state = read_until(&mut frames, response_to("s-1")).await;
    assert_eq!(state["success"], true, "{state}");
    println!("get_state ok: sessionId {}", state["data"]["sessionId"]);

    let listed = read_until(&mut frames, response_to("c-1")).await;
    let raw = listed["data"]["commands"].as_array().cloned().expect("command list");
    let live = Omp.catalog().parse_live(&raw);
    let builtins = live
        .iter()
        .filter(|command| command.source == CommandSource::Builtin)
        .count();
    println!("get_available_commands: {} commands, {builtins} builtins", live.len());

    assert!(builtins > 10, "builtins are listed");
    let review = live
        .iter()
        .find(|command| command.name == "review")
        .expect("/review is listed");
    assert_eq!(review.source, CommandSource::Custom);

    drop(process.stdin);
    let started = Instant::now();
    let exit = tokio::time::timeout(Duration::from_secs(10), process.child.wait())
        .await
        .expect("omp exits after stdin closes")
        .unwrap();
    println!("raw session exited {exit:?} after {:?}", started.elapsed());
}

#[tokio::test]
#[ignore = "needs a real omp; run with HARNESS_LIVE_OMP=1 and --ignored"]
async fn live_omp_handshake_state_commands_and_shutdown() {
    if !enabled() {
        println!("HARNESS_LIVE_OMP is not 1; skipped");
        return;
    }

    let binary = resolve_binary(&Env::from_process().unwrap(), "omp", None).expect("omp is installed");
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".omp/agent")).unwrap();
    std::fs::write(home.path().join(".omp/agent/models.yml"), OFFLINE_MODELS).unwrap();
    let env = Env::for_tests(home.path());
    let spawner = Arc::new(IsolatedSpawner {
        home: home.path().to_path_buf(),
    });

    let spec = Omp
        .launch_spec(&harness_business::harness::LaunchRequest {
            binary: &binary,
            project: project.path(),
            resume: None,
            approval_override: None,
            trust_once: false,
        })
        .unwrap();
    println!("command: {} {:?}", spec.program.display(), spec.args);
    raw_exchange(&spawner, &spec).await;

    // The same through the session engine.
    let manager = AgentManager::new();
    let sink = CollectingSink::new();
    let request = AgentStartRequest {
        harness: HarnessId::Omp,
        project: project.path().to_string_lossy().into_owned(),
        resume: None,
        approval_override: None,
        trust_once: false,
    };
    let deps = AgentDeps {
        harness: &Omp,
        env: &env,
        spawner,
        binary,
    };

    let info = manager
        .start(request, deps, sink.clone())
        .await
        .expect("session starts");
    println!("session: {info:?}");
    assert_eq!(info.protocol_version, Some(1));

    let state = manager.send(&info.key, AgentAction::GetState).await.expect("get_state");
    assert!(matches!(state, AgentReply::State { .. }));
    sink.wait_for(Duration::from_secs(5), |event| {
        matches!(event, AgentEvent::CommandsUpdated { .. })
    })
    .await
    .expect("catalog emitted");

    let started = Instant::now();
    manager.stop(&info.key).await.unwrap();
    let elapsed = started.elapsed();
    println!("engine session stopped after {elapsed:?}");
    assert!(elapsed < Duration::from_secs(10));

    let events = sink.events();
    println!("events: {:?}", events.iter().map(event_kind).collect::<Vec<_>>());
    assert!(matches!(
        events.first(),
        Some(AgentEvent::Ready {
            protocol_version: Some(1)
        })
    ));
    assert!(matches!(events.last(), Some(AgentEvent::Exited { .. })));
    assert!(home.path().join(".omp").exists(), "omp used the temporary home");
}

fn event_kind(event: &AgentEvent) -> String {
    serde_json::to_value(event).unwrap()["type"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}
