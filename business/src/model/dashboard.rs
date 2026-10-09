//! Harness Dashboard data through its read-only MCP server (`dashboard_query`,
//! `dashboard_availability`).
//!
//! The webview names a [`DashboardTool`] and passes its arguments as JSON; [`DashboardQuery::parse`]
//! checks them against the tool's typed arguments ([`super::dashboard_args`]) and the dashboard's
//! limits before anything is sent to `harness-dashboard mcp`.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use ts_rs::TS;

use super::dashboard_args::{
    BranchCostArgs, BreakdownArgs, FailuresArgs, MAX_DASHBOARD_ID_CHARS, MAX_DASHBOARD_NAME_CHARS,
    MAX_DASHBOARD_PATH_CHARS, MAX_DASHBOARD_REF_CHARS, MAX_DASHBOARD_ROWS, MAX_DASHBOARD_TIME_CHARS,
    MAX_DASHBOARD_TIPS, NoArgs, SessionCostArgs, TipsArgs, UsageSummaryArgs,
};
use crate::error::{BusinessError, Result};

/// A read-only tool of the dashboard's MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DashboardTool {
    /// Cost, tokens, sessions and prompts for a range, with the change against the period before.
    UsageSummary,
    /// Cost and tokens split by a dimension, most expensive first.
    Breakdown,
    /// Cost of one session, its subagents included.
    SessionCost,
    /// Cost of the work on one git branch over its whole life.
    BranchCost,
    /// Subscription plan-limit windows as last recorded by the dashboard.
    Limits,
    /// Spending caps set in the dashboard and how they stand.
    BudgetStatus,
    /// Why agent tool calls failed.
    Failures,
    /// Rule-based suggestions to spend less.
    Tips,
}

impl DashboardTool {
    /// Every tool, in the dashboard's order.
    pub const ALL: [Self; 8] = [
        Self::UsageSummary,
        Self::Breakdown,
        Self::SessionCost,
        Self::BranchCost,
        Self::Limits,
        Self::BudgetStatus,
        Self::Failures,
        Self::Tips,
    ];

    /// The MCP tool name (`tools/call` `name`).
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::UsageSummary => "usage_summary",
            Self::Breakdown => "breakdown",
            Self::SessionCost => "session_cost",
            Self::BranchCost => "branch_cost",
            Self::Limits => "limits",
            Self::BudgetStatus => "budget_status",
            Self::Failures => "failures",
            Self::Tips => "tips",
        }
    }

    /// The tool with this MCP name.
    pub fn from_wire_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.wire_name() == name)
    }

    /// Largest accepted `limit`, or `None` when the tool takes no `limit`.
    pub fn max_limit(self) -> Option<u32> {
        match self {
            Self::Breakdown | Self::Failures => Some(MAX_DASHBOARD_ROWS),
            Self::Tips => Some(MAX_DASHBOARD_TIPS),
            _ => None,
        }
    }
}

/// Arguments of `dashboard_query`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DashboardQuery {
    /// The tool to call.
    pub tool: DashboardTool,
    /// The tool's arguments (`UsageSummaryArgs`, `BreakdownArgs`, …; `{}` or `null` for
    /// `limits` and `budget_status`). Unknown fields are rejected.
    pub args: JsonValue,
}

/// Result of `dashboard_query`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DashboardResult {
    /// The tool that answered.
    pub tool: DashboardTool,
    /// The tool's `structuredContent`; `null` when `isError` is set.
    pub data: JsonValue,
    /// The text content when there is no structured content (the error message of a failed call).
    pub text: Option<String>,
    /// True when the tool reported an error it can explain (unknown session, no usage yet, …).
    pub is_error: bool,
}

/// Whether the dashboard can answer queries (`dashboard_availability`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum DashboardAvailability {
    /// `harness-dashboard mcp` started and listed its tools.
    Available,
    /// No `harness-dashboard` binary was found.
    NotInstalled,
    /// The binary exists but the MCP server could not be used.
    Failed {
        /// Short, secret-free reason.
        message: String,
    },
}

/// Validated, typed arguments of one dashboard tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DashboardArgs {
    /// `usage_summary`.
    UsageSummary(UsageSummaryArgs),
    /// `breakdown`.
    Breakdown(BreakdownArgs),
    /// `session_cost`.
    SessionCost(SessionCostArgs),
    /// `branch_cost`.
    BranchCost(BranchCostArgs),
    /// `limits` (no arguments).
    Limits,
    /// `budget_status` (no arguments).
    BudgetStatus,
    /// `failures`.
    Failures(FailuresArgs),
    /// `tips`.
    Tips(TipsArgs),
}

impl DashboardQuery {
    /// Parses `args` into the tool's typed arguments and checks the dashboard's limits.
    ///
    /// Fails with [`BusinessError::InvalidInput`] on unknown fields, wrong types or values, texts
    /// over their limit, a `limit` out of range, `from`/`to` without `range: "custom"` (or a custom
    /// range without either), and a `session_cost` without `session_id`/`cwd` or a `branch_cost`
    /// without `branch`.
    pub fn parse(&self) -> Result<DashboardArgs> {
        let args = match &self.args {
            JsonValue::Null => JsonValue::Object(Map::new()),
            JsonValue::Object(_) => self.args.clone(),
            _ => return Err(invalid(self.tool, "arguments must be an object")),
        };

        let parsed = match self.tool {
            DashboardTool::UsageSummary => DashboardArgs::UsageSummary(typed(self.tool, args)?),
            DashboardTool::Breakdown => DashboardArgs::Breakdown(typed(self.tool, args)?),
            DashboardTool::SessionCost => DashboardArgs::SessionCost(typed(self.tool, args)?),
            DashboardTool::BranchCost => DashboardArgs::BranchCost(typed(self.tool, args)?),
            DashboardTool::Failures => DashboardArgs::Failures(typed(self.tool, args)?),
            DashboardTool::Tips => DashboardArgs::Tips(typed(self.tool, args)?),
            DashboardTool::Limits => {
                typed::<NoArgs>(self.tool, args)?;
                DashboardArgs::Limits
            }
            DashboardTool::BudgetStatus => {
                typed::<NoArgs>(self.tool, args)?;
                DashboardArgs::BudgetStatus
            }
        };

        parsed.validate()?;
        Ok(parsed)
    }
}

impl DashboardArgs {
    /// The tool these arguments belong to.
    pub fn tool(&self) -> DashboardTool {
        match self {
            Self::UsageSummary(_) => DashboardTool::UsageSummary,
            Self::Breakdown(_) => DashboardTool::Breakdown,
            Self::SessionCost(_) => DashboardTool::SessionCost,
            Self::BranchCost(_) => DashboardTool::BranchCost,
            Self::Limits => DashboardTool::Limits,
            Self::BudgetStatus => DashboardTool::BudgetStatus,
            Self::Failures(_) => DashboardTool::Failures,
            Self::Tips(_) => DashboardTool::Tips,
        }
    }

    /// The `arguments` object of the MCP `tools/call`: the dashboard's field names, absent fields
    /// left out.
    pub fn to_arguments(&self) -> Result<Map<String, JsonValue>> {
        let value = match self {
            Self::UsageSummary(args) => serde_json::to_value(args),
            Self::Breakdown(args) => serde_json::to_value(args),
            Self::SessionCost(args) => serde_json::to_value(args),
            Self::BranchCost(args) => serde_json::to_value(args),
            Self::Failures(args) => serde_json::to_value(args),
            Self::Tips(args) => serde_json::to_value(args),
            Self::Limits | Self::BudgetStatus => serde_json::to_value(NoArgs {}),
        };

        match value {
            Ok(JsonValue::Object(mut map)) => {
                map.retain(|_, field| !field.is_null());
                Ok(map)
            }
            _ => Err(BusinessError::Internal(format!(
                "{} arguments do not serialize to an object",
                self.tool().wire_name()
            ))),
        }
    }

    fn validate(&self) -> Result<()> {
        let tool = self.tool();
        let arguments = self.to_arguments()?;

        for (key, field) in &arguments {
            check_text(tool, key, field)?;
        }

        check_range(tool, &arguments)?;
        check_limit(tool, &arguments)?;
        self.check_required()
    }

    /// The targets the app always names, because the dashboard's fallback (its own working
    /// directory) would be the app's.
    fn check_required(&self) -> Result<()> {
        match self {
            Self::SessionCost(args) if is_blank(&args.session_id) && is_blank(&args.cwd) => {
                Err(invalid(self.tool(), "session_id or cwd is required"))
            }
            Self::BranchCost(args) if is_blank(&args.branch) => Err(invalid(self.tool(), "branch is required")),
            _ => Ok(()),
        }
    }
}

fn typed<T: DeserializeOwned>(tool: DashboardTool, args: JsonValue) -> Result<T> {
    serde_json::from_value(args).map_err(|error| invalid(tool, &error.to_string()))
}

fn invalid(tool: DashboardTool, problem: &str) -> BusinessError {
    BusinessError::InvalidInput(format!("dashboard {}: {problem}", tool.wire_name()))
}

fn is_blank(text: &Option<String>) -> bool {
    text.as_deref().is_none_or(|text| text.trim().is_empty())
}

/// The longest text the dashboard accepts for an argument name (the same in every tool).
fn max_text_chars(key: &str) -> Option<usize> {
    match key {
        "from" | "to" => Some(MAX_DASHBOARD_TIME_CHARS),
        "provider" | "tag" => Some(MAX_DASHBOARD_NAME_CHARS),
        "model" | "user" | "tool" => Some(MAX_DASHBOARD_ID_CHARS),
        "session_id" | "branch" => Some(MAX_DASHBOARD_REF_CHARS),
        "project" | "cwd" => Some(MAX_DASHBOARD_PATH_CHARS),
        _ => None,
    }
}

fn check_text(tool: DashboardTool, key: &str, field: &JsonValue) -> Result<()> {
    let (Some(text), Some(max)) = (field.as_str(), max_text_chars(key)) else {
        return Ok(());
    };

    // The dashboard counts JavaScript string length: UTF-16 code units.
    if text.encode_utf16().count() > max {
        return Err(invalid(tool, &format!("{key} is longer than {max} characters")));
    }

    Ok(())
}

fn check_range(tool: DashboardTool, arguments: &Map<String, JsonValue>) -> Result<()> {
    let custom = arguments.get("range").and_then(JsonValue::as_str) == Some("custom");
    let has_bound = ["from", "to"].iter().any(|key| {
        arguments
            .get(*key)
            .and_then(JsonValue::as_str)
            .is_some_and(|text| !text.is_empty())
    });

    if custom && !has_bound {
        return Err(invalid(tool, "range custom needs from, to or both"));
    }

    if !custom && (arguments.contains_key("from") || arguments.contains_key("to")) {
        return Err(invalid(tool, "from and to apply only to range custom"));
    }

    Ok(())
}

fn check_limit(tool: DashboardTool, arguments: &Map<String, JsonValue>) -> Result<()> {
    let (Some(limit), Some(max)) = (arguments.get("limit").and_then(JsonValue::as_u64), tool.max_limit()) else {
        return Ok(());
    };

    if limit == 0 || limit > u64::from(max) {
        return Err(invalid(tool, &format!("limit must be between 1 and {max}")));
    }

    Ok(())
}
