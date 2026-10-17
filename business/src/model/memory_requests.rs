//! An agent asking to run a script this machine has not approved: Harness Memory writes a run
//! request file, the app shows it to the user (`memory_script_requests_listen`) and writes the
//! answer back (`memory_script_decide`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{ScriptScope, ScriptSummary};

/// One run request, as the server wrote it to `<data>/harness-memory/requests/<requestId>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScriptRunRequest {
    /// 32 lowercase hex characters, the file's name without `.json`.
    pub request_id: String,
    /// RFC 3339, when the server wrote the request (it waits 45 s for an answer), by the server's
    /// clock.
    pub created_at: String,
    /// RFC 3339 (UTC, whole seconds), when this app first saw the request, by its own clock. The
    /// answer window ends 45 s after the earlier of the two, so a clock that runs ahead on either
    /// side cannot stretch it.
    pub received_at: String,
    /// The MCP client that asked (`clientInfo.name`, e.g. `claude-code`); empty when it did not
    /// name itself.
    pub client: String,
    /// The script as `script_list` describes it.
    pub script: ScriptSummary,
    /// The code the server would run.
    pub body: String,
    /// The interpreter (`sh`, `bash`, `python`, `node`, `bun`).
    pub language: String,
    /// The arguments the agent passes.
    pub args: Vec<String>,
    /// The folder the script would run in; empty when the server does not say.
    pub cwd: String,
    /// The standard input the agent passes; `None` for none.
    pub stdin: Option<ScriptStdin>,
    /// The last version of the same script this machine approved, for a diff; `None` for a script
    /// never approved here.
    pub previous: Option<ScriptPreviousVersion>,
}

/// The standard input of a run, cut to a preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScriptStdin {
    /// The start of the input, as the server cut it.
    pub preview: String,
    /// The size of the whole input.
    #[ts(type = "number")]
    pub bytes: u64,
}

/// An earlier approved version of a script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScriptPreviousVersion {
    /// The code as approved.
    pub body: String,
    /// Its interpreter.
    pub language: String,
    /// RFC 3339, when it was approved.
    pub trusted_at: String,
    /// The name it had, when the server says (a rename shows).
    #[serde(default)]
    pub name: Option<String>,
    /// The scope it had, when the server says.
    #[serde(default)]
    pub scope: Option<ScriptScope>,
    /// How it was approved; `approved` when the server does not say.
    #[serde(default)]
    #[ts(inline)]
    pub origin: ScriptPreviousOrigin,
}

/// How an earlier version of a script came to be approved on this machine.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ScriptPreviousOrigin {
    /// An agent saved it on this machine (`script_save`).
    Saved,
    /// The user approved it (in the app or with `harness-memory trust`).
    #[default]
    Approved,
}

/// What the app tells the page about run requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum ScriptRequestEvent {
    /// A new request (also sent again, for each one still waiting, when a page starts listening).
    Request {
        /// The request.
        request: Box<ScriptRunRequest>,
    },
    /// The request file is gone: the server got an answer (maybe from another window or the
    /// terminal) or stopped waiting.
    Gone {
        /// The request that is gone.
        request_id: String,
    },
}
