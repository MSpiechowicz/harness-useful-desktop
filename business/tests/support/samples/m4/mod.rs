//! M4 fixtures for the Ecosystem tab, the dashboard widgets and extension actions
//! (`ui/src/lib/ipc/mock/fixtures/ecosystem/`). They are synthetic: detection depends on real
//! binaries, dashboard data on a real usage database and CLI results on real harness runs. The
//! dashboard results follow the `structuredContent` the dashboard's MCP tools return
//! (`harness-useful-dashboard/src/mcp/tools.ts`): money in USD rounded to 4 decimals, ratios to 3,
//! times in ISO 8601 UTC, and fields the dashboard leaves `undefined` omitted. The clock is
//! 2026-10-09T12:30:00Z on a machine in UTC+2.

mod cli;
mod ecosystem;
mod health;
mod usage;

use harness_business::model::{DashboardResult, DashboardTool};
use serde::Serialize;
use serde_json::Value;

fn to_json(value: impl Serialize) -> Value {
    serde_json::to_value(value).expect("DTOs always serialize")
}

/// Money as the dashboard rounds it (`usd` in `src/core/reports.ts`).
fn usd(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

/// A ratio as the dashboard rounds it (`ratio` in `src/core/reports.ts`).
fn ratio(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// A successful tool result carrying `data` as its structured content.
fn result(tool: DashboardTool, data: Value) -> Value {
    to_json(DashboardResult {
        tool,
        data,
        text: None,
        is_error: false,
    })
}

/// The cost note most tools append.
const COST_NOTE: &str = "Costs are USD at API-equivalent list prices, not what a subscription bills.";

/// Every M4 fixture path (relative to the fixtures directory) with its content.
pub fn m4_fixtures() -> Vec<(&'static str, Value)> {
    let mut fixtures = vec![("ecosystem/status.json", ecosystem::status())];
    fixtures.extend(usage::fixtures());
    fixtures.extend(health::fixtures());
    fixtures.extend(cli::fixtures());
    fixtures
}
