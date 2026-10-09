//! Live smoke tests of the ecosystem integration (ignored by default).
//!
//! - `HARNESS_LIVE_DASHBOARD=1 cargo test -p harness_business --test live_ecosystem live_dashboard
//!   -- --ignored --nocapture` starts the installed `harness-dashboard mcp` (read-only: the server
//!   only reads its database) and calls `usage_summary`. Only field names and counts are printed,
//!   never values. Skipped with a message when the dashboard is not installed.
//! - `HARNESS_LIVE_OMP_PLUGIN=1 cargo test -p harness_business --test live_ecosystem
//!   live_omp_plugin -- --ignored --nocapture` runs the installed `omp plugin` with `HOME` pointed
//!   at a temporary directory and the XDG, `PI_*` and `OMP_*` variables removed, so the real
//!   `~/.omp` is never read or written. OMP 18.6.1 ignores `--dry-run` for `upgrade` (it would
//!   upgrade for real), which is why even that "read-only" check runs in the temporary home.

use std::path::PathBuf;

use harness_business::ecosystem::{Dashboard, DashboardOptions, resolve_dashboard};
use harness_business::env::Env;
use harness_business::harness::{CliRun, run_cli_action};
use harness_business::harnesses::Omp;
use harness_business::model::{CliResult, DashboardQuery, DashboardTool, ExtensionAction};
use harness_business::process::{
    BoxFuture, ExecOutput, ExecSpec, ProcessRunner, TokioRunner, TokioSpawner, resolve_binary,
};
use serde_json::Value;

fn enabled(variable: &str) -> bool {
    std::env::var(variable).as_deref() == Ok("1")
}

/// `name: type (count)` for each top-level field; values are never printed.
fn shape(data: &Value) -> Vec<String> {
    let Some(fields) = data.as_object() else {
        return vec![format!("<{}>", kind(data))];
    };

    fields
        .iter()
        .map(|(name, value)| match value {
            Value::Array(items) => format!("{name}: array ({} items)", items.len()),
            Value::Object(map) => format!("{name}: object ({} fields)", map.len()),
            other => format!("{name}: {}", kind(other)),
        })
        .collect()
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[tokio::test]
#[ignore = "needs HARNESS_LIVE_DASHBOARD=1 and an installed harness-dashboard"]
async fn live_dashboard() {
    if !enabled("HARNESS_LIVE_DASHBOARD") {
        eprintln!("skipped: set HARNESS_LIVE_DASHBOARD=1");
        return;
    }

    let env = Env::from_process().expect("environment");
    let Some(binary) = resolve_dashboard(&env) else {
        eprintln!("skipped: harness-dashboard is not installed");
        return;
    };
    println!("binary: {}", binary.path.display());

    let dashboard = Dashboard::new(std::sync::Arc::new(TokioSpawner), DashboardOptions::default());
    let availability = dashboard.availability(&env).await;
    println!("availability: {availability:?}");

    let query = DashboardQuery {
        tool: DashboardTool::UsageSummary,
        args: serde_json::json!({ "range": "7d" }),
    };
    let result = dashboard.query(&env, &query).await.expect("usage_summary answers");
    println!("usage_summary isError: {}", result.is_error);
    println!("usage_summary has text: {}", result.text.is_some());
    for line in shape(&result.data) {
        println!("  {line}");
    }

    dashboard.shutdown().await;
    assert!(!result.is_error || result.text.is_some());
}

/// Runs every command with an isolated home (see the module docs).
struct IsolatedRunner {
    home: PathBuf,
}

impl ProcessRunner for IsolatedRunner {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, harness_business::Result<ExecOutput>> {
        let isolated = spec
            .clone()
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env_remove_prefix("XDG_")
            .env_remove_prefix("PI_")
            .env_remove_prefix("OMP_");

        Box::pin(async move { TokioRunner.run(&isolated).await })
    }
}

fn report(label: &str, result: &CliResult) {
    println!(
        "{label}: ok={} exitCode={:?} timedOut={} summary={:?}",
        result.ok, result.exit_code, result.timed_out, result.summary
    );
}

#[tokio::test]
#[ignore = "needs HARNESS_LIVE_OMP_PLUGIN=1 and an installed omp"]
async fn live_omp_plugin() {
    if !enabled("HARNESS_LIVE_OMP_PLUGIN") {
        eprintln!("skipped: set HARNESS_LIVE_OMP_PLUGIN=1");
        return;
    }

    let real = Env::from_process().expect("environment");
    let Some(binary) = resolve_binary(&real, "omp", None) else {
        eprintln!("skipped: omp is not installed");
        return;
    };

    let home = tempfile::tempdir().expect("temporary home");
    let env = Env {
        home: home.path().to_path_buf(),
        ..real
    };
    let runner = IsolatedRunner {
        home: home.path().to_path_buf(),
    };

    let run = async |action: ExtensionAction, dry_run: bool| {
        run_cli_action(CliRun {
            harness: &Omp,
            env: &env,
            runner: &runner,
            binary: &binary,
            action: &action,
            dry_run,
            project: None,
        })
        .await
        .expect("omp plugin runs")
    };

    let upgrade = run(ExtensionAction::PluginUpgrade { name: None }, false).await;
    report("upgrade", &upgrade);
    assert!(upgrade.ok, "nothing to upgrade in an empty home");

    let preview = run(ExtensionAction::PluginUpgrade { name: None }, true).await;
    report("upgrade (dry run)", &preview);
    assert_eq!(
        preview.exit_code, None,
        "OMP cannot preview an upgrade, so nothing runs"
    );
    assert!(preview.argv.iter().all(|arg| arg != "--dry-run"));
}
