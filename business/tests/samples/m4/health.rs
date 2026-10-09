//! Health fixtures: plan `limits`, `budget_status` (one cap near its limit), tool `failures` over
//! 7 days and `tips` over 30 days.

use harness_business::model::DashboardTool;
use serde_json::{Value, json};

use super::{COST_NOTE, ratio, result, usd};

const NOW: &str = "2026-10-09T12:30:00.000Z";
const DEMO_PROJECT: &str = "/home/demo/Projects/demo";

/// Claude's 5-hour and weekly windows (fresh), Codex's (one stale, one reset since its reading).
fn limits() -> Value {
    json!({
        "windows": [
            {
                "provider": "claude",
                "plan": "max",
                "window": "5h",
                "usedPercent": 42.5,
                "resetsAt": "2026-10-09T15:00:00.000Z",
                "observedAt": "2026-10-09T12:27:00.000Z",
                "ageMinutes": 3,
                "status": "fresh",
            },
            {
                "provider": "claude",
                "plan": "max",
                "window": "7d",
                "usedPercent": 61.2,
                "resetsAt": "2026-10-12T08:00:00.000Z",
                "observedAt": "2026-10-09T12:27:00.000Z",
                "ageMinutes": 3,
                "status": "fresh",
            },
            {
                "provider": "codex",
                "plan": "pro",
                "window": "5h",
                "usedPercent": null,
                "resetsAt": null,
                "observedAt": "2026-10-09T06:10:00.000Z",
                "ageMinutes": 380,
                "status": "reset since reading",
            },
            {
                "provider": "codex",
                "plan": "pro",
                "window": "7d",
                "usedPercent": 18.0,
                "resetsAt": "2026-10-14T06:10:00.000Z",
                "observedAt": "2026-10-09T06:10:00.000Z",
                "ageMinutes": 380,
                "status": "stale",
            },
        ],
        "note": "Readings older than 15 minutes are stale. The dashboard refreshes them every few minutes while it runs.",
    })
}

/// A cap as `budgetRows` in `src/core/reports.ts` reports it.
fn budget(scope: &str, project: Option<&str>, period: &str, cap: f64, spent: f64, projected: Option<f64>) -> Value {
    let fraction = spent / cap;
    let status = if fraction >= 1.0 {
        "over"
    } else if fraction >= 0.8 {
        "warning"
    } else {
        "ok"
    };

    let mut row = json!({ "scope": scope });
    if let Some(project) = project {
        row["project"] = json!(project);
    }

    row["period"] = json!(period);
    row["capUsd"] = json!(usd(cap));
    row["spentUsd"] = json!(usd(spent));
    row["usedPercent"] = json!((fraction * 1000.0).round() / 10.0);
    row["remainingUsd"] = json!(usd((cap - spent).max(0.0)));
    if let Some(projected) = projected {
        row["projectedUsd"] = json!(usd(projected));
    }

    row["status"] = json!(status);
    row
}

fn budget_status() -> Value {
    json!({
        "user": "demo",
        "budgets": [
            budget("daily", None, "2026-10-09", 10.0, 4.8312, None),
            budget("monthly", None, "2026-10", 150.0, 61.2304, Some(210.9047)),
            budget("project", Some(DEMO_PROJECT), "2026-10", 40.0, 34.1077, Some(117.4821)),
        ],
        "note": COST_NOTE,
    })
}

/// A tool's failure counts: calls, failed, declined (`errorRate` excludes declined calls).
fn tool_row(tool: &str, calls: u64, failed: u64, declined: u64, main_cause: &str) -> Value {
    json!({
        "tool": tool,
        "calls": calls,
        "failed": failed,
        "declined": declined,
        "errorRate": ratio(failed as f64 / (calls - declined) as f64),
        "mainCause": main_cause,
    })
}

fn failures() -> Value {
    let causes = [
        ("exit_code", 21, vec!["bash"]),
        ("edit_mismatch", 9, vec!["edit"]),
        ("not_found", 6, vec!["read", "bash"]),
        ("rejected", 4, vec!["bash"]),
        ("timeout", 2, vec!["bash"]),
    ];
    let total: u64 = causes.iter().map(|cause| cause.1).sum();
    let (calls, failed, declined) = (2914_u64, 38_u64, 4_u64);

    json!({
        "range": { "name": "7d", "from": "2026-10-02T22:00:00.000Z", "to": NOW },
        "totals": {
            "calls": calls,
            "failed": failed,
            "declined": declined,
            "interruptedPrompts": 3,
            "errorRate": ratio(failed as f64 / (calls - declined) as f64),
        },
        "causes": causes
            .iter()
            .map(|(cause, count, tools)| {
                json!({ "cause": cause, "count": count, "share": ratio(*count as f64 / total as f64), "tools": tools })
            })
            .collect::<Vec<_>>(),
        "tools": [
            tool_row("bash", 1288, 27, 4, "exit_code"),
            tool_row("edit", 604, 9, 0, "edit_mismatch"),
            tool_row("read", 1022, 2, 0, "not_found"),
        ],
        "recent": [
            {
                "time": "2026-10-09T12:14:31.000Z",
                "tool": "bash",
                "cause": "exit_code",
                "error": "error[E0308]: mismatched types: expected `Result<Vec<Record>, String>`, found `Vec<Record>`",
                "input": "cargo test",
                "provider": "omp",
                "project": "demo",
                "sessionId": "omp:0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57",
            },
            {
                "time": "2026-10-09T11:52:08.000Z",
                "tool": "edit",
                "cause": "edit_mismatch",
                "error": "Could not find the text to replace in src/lib.rs. Verify the content matches the file.",
                "input": "src/lib.rs",
                "provider": "omp",
                "project": "demo",
                "sessionId": "omp:0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57",
            },
            {
                "time": "2026-10-08T16:31:44.000Z",
                "tool": "read",
                "cause": "not_found",
                "error": "ENOENT: no such file or directory, open 'docs/usage.md'",
                "input": "docs/usage.md",
                "provider": "pi",
                "project": "demo",
                "sessionId": "pi:8c2d4e6f-1a3b-4c5d-8e7f-9a0b1c2d3e4f",
            },
        ],
        "apiErrors": {
            "failedRequests": 5,
            "rate": 0.003,
            "classes": [{ "class": "rate_limit", "count": 3 }, { "class": "overloaded", "count": 2 }],
        },
    })
}

fn tips() -> Value {
    json!({
        "range": { "name": "30d", "from": "2026-09-09T22:00:00.000Z", "to": NOW },
        "tips": [
            {
                "id": "premium-small-prompts",
                "severity": "info",
                "category": "models",
                "title": "64 small prompts on a premium model",
                "detail": "64 short prompts ($11.82) ran on claude-opus-5-5. Quick questions often work just as well on a faster, cheaper model or at lower effort.",
                "impactUsdPerMonth": 7.4512,
            },
            {
                "id": "cache-expired",
                "severity": "info",
                "category": "cache",
                "title": "The cache expired 23 times during a pause",
                "detail": "The prompt cache lives 5 minutes. After a longer pause the next call writes the whole context again, which cost about $3.17 here. Finish a step before a break, or compact a long session before stepping away.",
                "impactUsdPerMonth": 2.219,
            },
            {
                "id": "long-session",
                "severity": "info",
                "category": "context",
                "title": "Very long session: 412 calls ($9.64)",
                "detail": "\"Refactor the record parser\" ran for 412 model calls. Context grows with every turn — split long efforts into focused sessions.",
                "impactUsdPerMonth": 1.928,
                "sessionId": "omp:01999e40-7d21-7f03-a4b6-c8d9e0f1a2b3",
            },
            {
                "id": "tool-loops",
                "severity": "warn",
                "category": "workflow",
                "title": "2 prompts with 150+ tool calls",
                "detail": "The busiest prompt triggered 187 tool calls. Long tool loops often mean a missing instruction or a flaky test — consider breaking the task up.",
                "promptId": "omp:01999e40-7d21-7f03-a4b6-c8d9e0f1a2b3:p17",
            },
        ],
    })
}

/// The `ecosystem/` health fixtures.
pub fn fixtures() -> Vec<(&'static str, Value)> {
    vec![
        ("ecosystem/limits.json", result(DashboardTool::Limits, limits())),
        (
            "ecosystem/budget_status.json",
            result(DashboardTool::BudgetStatus, budget_status()),
        ),
        ("ecosystem/failures.json", result(DashboardTool::Failures, failures())),
        ("ecosystem/tips.json", result(DashboardTool::Tips, tips())),
    ]
}
