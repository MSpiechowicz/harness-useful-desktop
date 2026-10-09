//! Spend fixtures: `usage_summary` for today, 7 and 30 days, `breakdown` by model and by harness,
//! `session_cost` (of the M2 chat session, and an error) and `branch_cost` (of the M3 branch).

use harness_business::model::{DashboardResult, DashboardTool};
use serde_json::{Value, json};

use super::{COST_NOTE, ratio, result, to_json, usd};

const NOW: &str = "2026-10-09T12:30:00.000Z";
const DEMO_PROJECT: &str = "/home/demo/Projects/demo";

/// The dashboard's id of the OMP chat session in `agent/state.json`.
const SESSION_ID: &str = "omp:0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57";

/// Tokens by type, as `tokensOf` in `src/mcp/tools.ts` reports them.
struct Tokens {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
    reasoning: u64,
}

impl Tokens {
    fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    fn cache_hit_rate(&self) -> f64 {
        self.cache_read as f64 / (self.input + self.cache_read + self.cache_write) as f64
    }

    fn to_json(&self) -> Value {
        json!({
            "total": self.total(),
            "input": self.input,
            "output": self.output,
            "cacheRead": self.cache_read,
            "cacheWrite": self.cache_write,
            "reasoning": self.reasoning,
        })
    }
}

/// One `usage_summary` period and the same span before it.
struct Period {
    range: &'static str,
    from: &'static str,
    previous_from: &'static str,
    cost: f64,
    tokens: Tokens,
    model_calls: u64,
    sessions: u64,
    prompts: u64,
    active_days: u64,
    first_activity: &'static str,
    previous: (f64, u64, u64, u64),
}

fn summary(period: &Period) -> Value {
    let (previous_cost, previous_tokens, previous_sessions, previous_prompts) = period.previous;

    json!({
        "range": { "name": period.range, "from": period.from, "to": NOW },
        "costUsd": usd(period.cost),
        "tokens": period.tokens.to_json(),
        "modelCalls": period.model_calls,
        "sessions": period.sessions,
        "prompts": period.prompts,
        "activeDays": period.active_days,
        "cacheHitRate": ratio(period.tokens.cache_hit_rate()),
        "costPerPromptUsd": usd(period.cost / period.prompts as f64),
        "firstActivity": period.first_activity,
        "lastActivity": "2026-10-09T12:27:09.000Z",
        "previousPeriod": {
            "from": period.previous_from,
            "to": period.from,
            "costUsd": usd(previous_cost),
            "tokens": previous_tokens,
            "sessions": previous_sessions,
            "prompts": previous_prompts,
            "costChange": ratio((period.cost - previous_cost) / previous_cost),
        },
        "note": COST_NOTE,
    })
}

fn periods() -> [Period; 3] {
    let tokens = |input, output, cache_read, cache_write, reasoning| Tokens {
        input,
        output,
        cache_read,
        cache_write,
        reasoning,
    };

    [
        Period {
            range: "today",
            from: "2026-10-08T22:00:00.000Z",
            previous_from: "2026-10-08T07:30:00.000Z",
            cost: 4.8312,
            tokens: tokens(182_400, 96_350, 3_912_000, 241_800, 21_400),
            model_calls: 214,
            sessions: 6,
            prompts: 41,
            active_days: 1,
            first_activity: "2026-10-09T06:12:44.000Z",
            previous: (6.2045, 5_310_220, 8, 55),
        },
        Period {
            range: "7d",
            from: "2026-10-02T22:00:00.000Z",
            previous_from: "2026-09-26T07:30:00.000Z",
            cost: 38.4127,
            tokens: tokens(1_422_000, 702_300, 30_120_500, 1_934_000, 160_200),
            model_calls: 1672,
            sessions: 47,
            prompts: 318,
            active_days: 6,
            first_activity: "2026-10-03T07:41:02.000Z",
            previous: (31.0852, 28_904_110, 39, 270),
        },
        Period {
            range: "30d",
            from: "2026-09-09T22:00:00.000Z",
            previous_from: "2026-08-11T07:30:00.000Z",
            cost: 142.9071,
            tokens: tokens(5_310_400, 2_588_900, 112_403_000, 7_205_300, 590_100),
            model_calls: 6215,
            sessions: 171,
            prompts: 1204,
            active_days: 22,
            first_activity: "2026-09-10T08:03:15.000Z",
            previous: (163.4410, 131_220_400, 190, 1351),
        },
    ]
}

/// One `breakdown` row before its share is known: key, cost, tokens, calls, sessions, prompts,
/// cache hit rate, last activity and whether its price is estimated.
type Row = (&'static str, f64, u64, u64, u64, u64, f64, &'static str, bool);

/// A 30-day `breakdown`; the rows add up to the 30-day summary.
fn breakdown(dimension: &str, rows: &[Row]) -> Value {
    let total_cost: f64 = rows.iter().map(|row| row.1).sum();
    let total_tokens: u64 = rows.iter().map(|row| row.2).sum();

    let rows: Vec<Value> = rows
        .iter()
        .map(
            |&(key, cost, tokens, calls, sessions, prompts, hit_rate, last, estimated)| {
                let mut row = json!({
                    "key": key,
                    "costUsd": usd(cost),
                    "share": ratio(cost / total_cost),
                    "tokens": tokens,
                    "modelCalls": calls,
                    "sessions": sessions,
                    "prompts": prompts,
                    "cacheHitRate": hit_rate,
                    "lastActivity": last,
                });
                if estimated {
                    row["estimatedPrice"] = json!(true);
                }
                row
            },
        )
        .collect();

    json!({
        "dimension": dimension,
        "range": { "name": "30d", "from": "2026-09-09T22:00:00.000Z", "to": NOW },
        "totalCostUsd": usd(total_cost),
        "totalTokens": total_tokens,
        "rows": rows,
        "note": COST_NOTE,
    })
}

fn by_model() -> Value {
    breakdown(
        "model",
        &[
            (
                "claude-opus-5-5",
                88.1203,
                71_220_000,
                3104,
                96,
                642,
                0.921,
                "2026-10-09T12:27:09.000Z",
                false,
            ),
            (
                "claude-sonnet-5",
                31.4410,
                34_105_600,
                1880,
                61,
                377,
                0.904,
                "2026-10-09T11:02:51.000Z",
                false,
            ),
            (
                "gpt-6-sol",
                17.2289,
                15_982_000,
                958,
                23,
                141,
                0.873,
                "2026-10-08T17:45:30.000Z",
                false,
            ),
            (
                "gemini-3-pro",
                6.1169,
                6_200_000,
                273,
                9,
                44,
                0.812,
                "2026-10-01T09:14:05.000Z",
                true,
            ),
        ],
    )
}

fn by_provider() -> Value {
    breakdown(
        "provider",
        &[
            (
                "omp",
                96.3302,
                85_400_000,
                4011,
                102,
                731,
                0.917,
                "2026-10-09T12:27:09.000Z",
                false,
            ),
            (
                "claude",
                30.1180,
                27_007_600,
                1302,
                38,
                301,
                0.908,
                "2026-10-08T19:20:12.000Z",
                false,
            ),
            (
                "pi",
                11.0410,
                9_800_000,
                612,
                20,
                118,
                0.889,
                "2026-10-07T15:33:47.000Z",
                false,
            ),
            (
                "codex",
                5.4179,
                5_300_000,
                290,
                11,
                54,
                0.861,
                "2026-10-04T10:08:22.000Z",
                false,
            ),
        ],
    )
}

/// The chat session: two subagents, two commits, no pull request yet.
fn session_cost() -> Value {
    let tokens = Tokens {
        input: 64_200,
        output: 38_900,
        cache_read: 1_802_000,
        cache_write: 96_400,
        reasoning: 7_300,
    };
    let (own_cost, subagent_cost) = (1.9284, 0.4126);
    let cost = own_cost + subagent_cost;
    let (added, removed) = (142, 37);

    json!({
        "sessionId": SESSION_ID,
        "resolvedBy": "session_id",
        "provider": "omp",
        "title": "Handle empty input in the record parser",
        "project": DEMO_PROJECT,
        "branch": "feat/empty-input",
        "started": "2026-10-09T09:58:12.000Z",
        "lastActivity": "2026-10-09T12:27:09.000Z",
        "costUsd": usd(cost),
        "ownCostUsd": usd(own_cost),
        "linesChanged": {
            "added": added,
            "removed": removed,
            "costPer100LinesUsd": usd(cost / f64::from(added + removed) * 100.0),
        },
        "tokens": tokens.to_json(),
        "modelCalls": 58,
        "prompts": 9,
        "subagents": { "count": 2, "costUsd": usd(subagent_cost), "tokens": 412_800 },
        "models": [
            { "model": "claude-opus-5-5", "costUsd": 1.6102, "tokens": 1_610_300 },
            { "model": "claude-sonnet-5", "costUsd": 0.3182, "tokens": 391_200 },
        ],
        "commits": 2,
        "pullRequestCount": 0,
        "pullRequests": [],
        "compactions": { "count": 1, "estimatedCostUsd": 0.0841 },
        "note": COST_NOTE,
    })
}

/// A session the dashboard has not scanned yet: a tool error, no structured content.
fn session_cost_error() -> Value {
    to_json(DashboardResult {
        tool: DashboardTool::SessionCost,
        data: Value::Null,
        text: Some(
            "No session 0199d000-0000-7000-8000-000000000001 in the database yet. The dashboard scans every 30 \
             seconds while it runs."
                .to_owned(),
        ),
        is_error: true,
    })
}

/// The checked-out branch of the demo project, with one open pull request.
fn branch_cost() -> Value {
    let cost = 7.8813;
    let (added, removed) = (318, 96);
    let commits = 5;

    json!({
        "branch": "feat/empty-input",
        "project": DEMO_PROJECT,
        "costUsd": usd(cost),
        "tokens": 7_310_400,
        "modelCalls": 203,
        "sessions": 4,
        "prompts": 31,
        "firstActivity": "2026-10-07T08:15:40.000Z",
        "lastActivity": "2026-10-09T12:27:09.000Z",
        "linesChanged": {
            "added": added,
            "removed": removed,
            "files": 7,
            "costPer100LinesUsd": usd(cost / f64::from(added + removed) * 100.0),
        },
        "commits": commits,
        "pullRequests": [{ "number": 12, "url": "https://github.com/demo/demo/pull/12" }],
        "costPerCommitUsd": usd(cost / f64::from(commits)),
        "costPerPullRequestUsd": usd(cost),
        "models": [
            { "model": "claude-opus-5-5", "costUsd": 6.0412, "tokens": 5_402_100, "linesChanged": 351 },
            { "model": "claude-sonnet-5", "costUsd": 1.8401, "tokens": 1_908_300, "linesChanged": 63 },
        ],
        "latestSessions": [
            {
                "id": SESSION_ID,
                "title": "Handle empty input in the record parser",
                "provider": "omp",
                "costUsd": 2.341,
                "subagents": 2,
                "lastActivity": "2026-10-09T12:27:09.000Z",
            },
            {
                "id": "omp:01999e40-7d21-7f03-a4b6-c8d9e0f1a2b3",
                "title": "Sketch the streaming parser",
                "provider": "omp",
                "costUsd": 3.9127,
                "lastActivity": "2026-10-08T16:40:02.000Z",
            },
            {
                "id": "pi:8c2d4e6f-1a3b-4c5d-8e7f-9a0b1c2d3e4f",
                "title": "Review the parser error messages",
                "provider": "pi",
                "costUsd": 1.6276,
                "lastActivity": "2026-10-07T14:05:33.000Z",
            },
        ],
        "topEditedFiles": [
            { "file": "src/lib.rs", "edits": 23 },
            { "file": "tests/parse.rs", "edits": 11 },
            { "file": "docs/guide.md", "edits": 4 },
        ],
        "note": COST_NOTE,
    })
}

/// The `ecosystem/` spend fixtures.
pub fn fixtures() -> Vec<(&'static str, Value)> {
    let [today, week, month] = periods();

    vec![
        (
            "ecosystem/usage_summary.json",
            result(DashboardTool::UsageSummary, summary(&today)),
        ),
        (
            "ecosystem/usage_summary.7d.json",
            result(DashboardTool::UsageSummary, summary(&week)),
        ),
        (
            "ecosystem/usage_summary.30d.json",
            result(DashboardTool::UsageSummary, summary(&month)),
        ),
        ("ecosystem/breakdown.json", result(DashboardTool::Breakdown, by_model())),
        (
            "ecosystem/breakdown.provider.json",
            result(DashboardTool::Breakdown, by_provider()),
        ),
        (
            "ecosystem/session_cost.json",
            result(DashboardTool::SessionCost, session_cost()),
        ),
        ("ecosystem/session_cost.error.json", session_cost_error()),
        (
            "ecosystem/branch_cost.json",
            result(DashboardTool::BranchCost, branch_cost()),
        ),
    ]
}
