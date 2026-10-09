//! Dashboard tool arguments: parsed into the tool's typed arguments and checked against the
//! dashboard's MCP input schemas (`harness-useful-dashboard/src/mcp/tools.ts`) before any call.

use harness_business::model::{
    DashboardArgs, DashboardDimension, DashboardQuery, DashboardRange, DashboardTool, IpcErrorCode, SessionCostArgs,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn query(tool: DashboardTool, args: Value) -> DashboardQuery {
    DashboardQuery { tool, args }
}

fn parse(tool: DashboardTool, args: Value) -> Result<DashboardArgs, String> {
    query(tool, args).parse().map_err(|error| {
        assert_eq!(error.code(), IpcErrorCode::InvalidInput, "{error}");
        error.to_string()
    })
}

/// The `tools/call` arguments the query would send.
fn arguments(tool: DashboardTool, args: Value) -> Value {
    let parsed = parse(tool, args).expect("valid arguments");
    Value::Object(parsed.to_arguments().expect("arguments serialize"))
}

fn rejected(tool: DashboardTool, args: Value) -> String {
    match parse(tool, args.clone()) {
        Ok(parsed) => panic!("{} accepted {args}: {parsed:?}", tool.wire_name()),
        Err(message) => message,
    }
}

#[test]
fn tools_use_the_dashboard_mcp_names() {
    let names: Vec<&str> = DashboardTool::ALL.iter().map(|tool| tool.wire_name()).collect();

    assert_eq!(
        names,
        [
            "usage_summary",
            "breakdown",
            "session_cost",
            "branch_cost",
            "limits",
            "budget_status",
            "failures",
            "tips"
        ]
    );

    for tool in DashboardTool::ALL {
        assert_eq!(DashboardTool::from_wire_name(tool.wire_name()), Some(tool));
    }
    assert_eq!(DashboardTool::from_wire_name("usageSummary"), None);
    assert_eq!(
        serde_json::to_value(DashboardTool::BudgetStatus).unwrap(),
        "budgetStatus"
    );
}

#[test]
fn arguments_keep_the_dashboard_field_names_and_drop_absent_fields() {
    assert_eq!(
        arguments(
            DashboardTool::SessionCost,
            json!({ "session_id": "0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57", "cwd": null })
        ),
        json!({ "session_id": "0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57" })
    );

    assert_eq!(
        arguments(
            DashboardTool::Breakdown,
            json!({ "dimension": "provider", "range": "7d", "kind": "bugfix", "limit": 5 })
        ),
        json!({ "dimension": "provider", "range": "7d", "kind": "bugfix", "limit": 5 })
    );

    assert_eq!(arguments(DashboardTool::UsageSummary, json!({})), json!({}));
}

#[test]
fn parsed_arguments_are_typed_per_tool() {
    let parsed = parse(
        DashboardTool::Breakdown,
        json!({ "dimension": "model", "range": "30d", "project": "/home/demo/Projects/demo" }),
    )
    .unwrap();

    let DashboardArgs::Breakdown(args) = &parsed else {
        panic!("not a breakdown: {parsed:?}");
    };
    assert_eq!(args.dimension, DashboardDimension::Model);
    assert_eq!(args.range, Some(DashboardRange::ThirtyDays));
    assert_eq!(parsed.tool(), DashboardTool::Breakdown);

    assert_eq!(
        parse(DashboardTool::SessionCost, json!({ "cwd": "/home/demo/Projects/demo" })).unwrap(),
        DashboardArgs::SessionCost(SessionCostArgs {
            session_id: None,
            cwd: Some("/home/demo/Projects/demo".to_owned()),
        })
    );
}

#[test]
fn tools_without_arguments_take_an_empty_object_or_null() {
    for tool in [DashboardTool::Limits, DashboardTool::BudgetStatus] {
        assert_eq!(arguments(tool, json!({})), json!({}));
        assert_eq!(arguments(tool, Value::Null), json!({}));
        rejected(tool, json!({ "range": "today" }));
    }

    assert_eq!(arguments(DashboardTool::UsageSummary, Value::Null), json!({}));
    rejected(DashboardTool::UsageSummary, json!([]));
    rejected(DashboardTool::Tips, json!("today"));
}

#[test]
fn fields_a_tool_does_not_declare_are_rejected() {
    let cases = [
        (DashboardTool::UsageSummary, json!({ "limit": 5 })),
        (DashboardTool::UsageSummary, json!({ "sessionId": "x" })),
        (
            DashboardTool::Breakdown,
            json!({ "dimension": "model", "tool": "bash" }),
        ),
        (DashboardTool::SessionCost, json!({ "sessionId": "0199c1f2" })),
        (DashboardTool::BranchCost, json!({ "branch": "main", "range": "7d" })),
        (DashboardTool::Failures, json!({ "user": "demo" })),
        (DashboardTool::Failures, json!({ "kind": "bugfix" })),
        (DashboardTool::Tips, json!({ "dimension": "model" })),
    ];

    for (tool, args) in cases {
        let message = rejected(tool, args);
        assert!(message.contains(tool.wire_name()), "{message}");
    }
}

#[test]
fn enum_values_follow_the_dashboard_constants() {
    for range in ["today", "7d", "30d", "month", "all"] {
        parse(DashboardTool::UsageSummary, json!({ "range": range })).unwrap();
    }
    for dimension in ["project", "model", "provider", "user", "skill", "agent", "tag", "kind"] {
        parse(DashboardTool::Breakdown, json!({ "dimension": dimension })).unwrap();
    }
    for kind in [
        "feature", "bugfix", "refactor", "tests", "docs", "research", "ops", "other",
    ] {
        parse(DashboardTool::Tips, json!({ "kind": kind })).unwrap();
    }

    rejected(DashboardTool::UsageSummary, json!({ "range": "yesterday" }));
    rejected(DashboardTool::UsageSummary, json!({ "range": "7D" }));
    rejected(DashboardTool::Breakdown, json!({ "dimension": "day" }));
    rejected(DashboardTool::Breakdown, json!({ "range": "7d" }));
    rejected(DashboardTool::Tips, json!({ "kind": "chore" }));
}

#[test]
fn texts_are_limited_like_the_dashboard_schema() {
    let cases = [
        (DashboardTool::UsageSummary, "provider", 100),
        (DashboardTool::UsageSummary, "project", 1000),
        (DashboardTool::UsageSummary, "model", 200),
        (DashboardTool::UsageSummary, "user", 200),
        (DashboardTool::UsageSummary, "tag", 100),
        (DashboardTool::SessionCost, "session_id", 300),
        (DashboardTool::SessionCost, "cwd", 1000),
        (DashboardTool::BranchCost, "branch", 300),
        (DashboardTool::BranchCost, "project", 1000),
        (DashboardTool::Failures, "tool", 200),
    ];

    for (tool, field, max) in cases {
        // branch_cost needs a branch beside the field under test.
        let with = |text: String| {
            let mut args = json!({ field: text });
            if tool == DashboardTool::BranchCost && field != "branch" {
                args["branch"] = json!("main");
            }
            args
        };

        parse(tool, with("a".repeat(max))).unwrap();

        let message = rejected(tool, with("a".repeat(max + 1)));
        assert!(message.contains(&format!("{field} is longer than {max}")), "{message}");
    }
}

#[test]
fn text_length_counts_utf16_units_like_javascript() {
    let custom = |from: String| json!({ "range": "custom", "from": from });

    parse(DashboardTool::UsageSummary, custom("😀".repeat(20))).unwrap();
    rejected(DashboardTool::UsageSummary, custom("😀".repeat(21)));
    parse(DashboardTool::UsageSummary, custom("ż".repeat(40))).unwrap();
}

#[test]
fn limits_stay_within_each_tools_maximum() {
    for (tool, base, max) in [
        (DashboardTool::Breakdown, json!({ "dimension": "model" }), 50),
        (DashboardTool::Failures, json!({}), 50),
        (DashboardTool::Tips, json!({}), 30),
    ] {
        let with_limit = |limit: Value| {
            let mut args = base.clone();
            args["limit"] = limit;
            args
        };

        parse(tool, with_limit(json!(1))).unwrap();
        parse(tool, with_limit(json!(max))).unwrap();
        rejected(tool, with_limit(json!(max + 1)));
        rejected(tool, with_limit(json!(0)));
        rejected(tool, with_limit(json!(-1)));
        rejected(tool, with_limit(json!(2.5)));
        rejected(tool, with_limit(json!("10")));
    }
}

#[test]
fn from_and_to_belong_to_a_custom_range() {
    let tool = DashboardTool::Tips;

    parse(tool, json!({ "range": "custom", "from": "2026-10-01" })).unwrap();
    parse(tool, json!({ "range": "custom", "to": "2026-10-09T12:00:00Z" })).unwrap();
    parse(
        tool,
        json!({ "range": "custom", "from": "2026-10-01", "to": "2026-10-09" }),
    )
    .unwrap();

    rejected(tool, json!({ "range": "custom" }));
    rejected(tool, json!({ "range": "custom", "from": "" }));
    rejected(tool, json!({ "range": "7d", "from": "2026-10-01" }));
    rejected(tool, json!({ "to": "2026-10-09" }));
}

#[test]
fn session_and_branch_costs_name_their_target() {
    rejected(DashboardTool::SessionCost, json!({}));
    rejected(DashboardTool::SessionCost, json!({ "session_id": "  " }));
    rejected(DashboardTool::BranchCost, json!({ "project": "demo" }));
    rejected(DashboardTool::BranchCost, json!({ "branch": "" }));

    parse(
        DashboardTool::SessionCost,
        json!({ "session_id": "0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57" }),
    )
    .unwrap();
    parse(
        DashboardTool::BranchCost,
        json!({ "branch": "feat/empty-input", "project": "demo" }),
    )
    .unwrap();
}

#[test]
fn queries_arrive_with_a_camel_case_tool_and_raw_arguments() {
    let query: DashboardQuery = serde_json::from_value(json!({
        "tool": "branchCost",
        "args": { "branch": "main" }
    }))
    .unwrap();

    assert_eq!(query.tool, DashboardTool::BranchCost);
    assert_eq!(query.args, json!({ "branch": "main" }));
}
