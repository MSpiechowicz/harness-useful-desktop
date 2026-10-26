//! The `result` frame: exactly one per turn, `success` or `error_*`, with the turn's main-loop
//! usage and the process's running totals (`total_cost_usd`, `modelUsage`). Its numbers are read
//! leniently: a float or an odd value never loses the result.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use super::messages::ApiUsage;
use super::{lenient, lenient_option, nullable};

/// The `subtype` of a successful turn (it may still be `is_error`, e.g. without a login).
pub const SUCCESS: &str = "success";

/// A `result` frame.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct ResultFrame {
    /// `success`, `error_during_execution`, `error_max_turns`, `error_max_budget_usd`, …
    #[serde(deserialize_with = "nullable")]
    pub subtype: String,
    #[serde(deserialize_with = "nullable")]
    pub is_error: bool,
    /// Model calls of the turn; 0 for a turn that never asked the model (a `shouldQuery: false`
    /// message, `/compact`, a local command).
    #[serde(deserialize_with = "lenient")]
    pub num_turns: u32,
    /// The final reply text, or the error text of an `is_error` success.
    pub result: Option<String>,
    /// The errors of an `error_*` result.
    #[serde(deserialize_with = "nullable")]
    pub errors: Vec<String>,
    /// Why the turn ended (`completed`, `aborted_streaming`, `aborted_tools`, `api_error`, …).
    pub terminal_reason: Option<String>,
    pub stop_reason: Option<String>,
    /// The local command that made the turn (`compact`).
    pub local_command: Option<String>,
    /// The HTTP status of a failed API call.
    #[serde(deserialize_with = "lenient_option")]
    pub api_error_status: Option<u16>,
    /// API-equivalent cost of the process so far, in US dollars.
    #[serde(deserialize_with = "lenient_option")]
    pub total_cost_usd: Option<f64>,
    /// The turn's main-loop usage, summed over its model calls (see [`ResultUsage::iterations`]).
    pub usage: Option<ResultUsage>,
    /// Totals of the process so far per model (main loop, subagents, compaction).
    #[serde(rename = "modelUsage", deserialize_with = "nullable")]
    pub model_usage: BTreeMap<String, ModelUsage>,
    #[serde(deserialize_with = "nullable")]
    pub permission_denials: Vec<PermissionDenial>,
    #[serde(deserialize_with = "lenient_option")]
    pub duration_ms: Option<u64>,
    #[serde(deserialize_with = "lenient_option")]
    pub duration_api_ms: Option<u64>,
    /// The first prompt the turn served (queue2.jsonl: a turn that read a steer names the prompt
    /// that started it).
    pub user_message_uuid: Option<String>,
    /// Every prompt the turn served, in order.
    #[serde(deserialize_with = "nullable")]
    pub user_message_uuids: Vec<String>,
    pub session_id: Option<String>,
}

impl ResultFrame {
    /// True for a `success` result that is not an error.
    pub fn succeeded(&self) -> bool {
        self.subtype == SUCCESS && !self.is_error
    }

    /// The prompts the turn served: `user_message_uuids`, else `user_message_uuid`.
    pub fn prompts(&self) -> Vec<String> {
        if !self.user_message_uuids.is_empty() {
            return self.user_message_uuids.clone();
        }

        self.user_message_uuid.iter().cloned().collect()
    }
}

/// `usage` of a result: the turn's sums and each model call (`iterations`, oldest first).
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct ResultUsage {
    #[serde(flatten)]
    pub sums: ApiUsage,
    #[serde(deserialize_with = "nullable")]
    pub iterations: Vec<ApiUsage>,
}

/// One model's totals in `modelUsage`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelUsage {
    #[serde(deserialize_with = "lenient")]
    pub input_tokens: u64,
    #[serde(deserialize_with = "lenient")]
    pub output_tokens: u64,
    #[serde(deserialize_with = "lenient")]
    pub cache_read_input_tokens: u64,
    #[serde(deserialize_with = "lenient")]
    pub cache_creation_input_tokens: u64,
    #[serde(deserialize_with = "lenient")]
    pub thinking_tokens: u64,
    #[serde(rename = "costUSD", deserialize_with = "lenient")]
    pub cost_usd: f64,
    #[serde(deserialize_with = "lenient_option")]
    pub context_window: Option<u64>,
    #[serde(deserialize_with = "lenient_option")]
    pub max_output_tokens: Option<u64>,
}

/// A tool call the turn denied.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct PermissionDenial {
    #[serde(deserialize_with = "nullable")]
    pub tool_name: String,
    #[serde(deserialize_with = "nullable")]
    pub tool_use_id: String,
    #[serde(deserialize_with = "nullable")]
    pub tool_input: Value,
}
