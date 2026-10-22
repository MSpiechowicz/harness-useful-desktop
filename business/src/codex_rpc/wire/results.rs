//! The results of the app's requests, read with [`Response::result`](super::Response::result):
//! the handshake, threads (start, resume, fork, read, list), turns and items pages, models,
//! skills, MCP server status and the account. Only the fields the app uses; the account's email
//! and the threads' rollout paths are never read.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::Value;

use super::items::{ThreadItem, Turn};
use crate::claude_stream::wire::{lenient_option, nullable};

/// `initialize`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InitializeResult {
    /// `<originator>/<codex version> (<os>; <arch>) …`.
    #[serde(deserialize_with = "nullable")]
    pub user_agent: String,
    #[serde(deserialize_with = "nullable")]
    pub codex_home: String,
    #[serde(deserialize_with = "nullable")]
    pub platform_os: String,
}

impl InitializeResult {
    /// The Codex version the user agent names (`…/0.154.0 (…`).
    pub fn version(&self) -> Option<&str> {
        let product = self.user_agent.split_whitespace().next()?;
        let version = product.rsplit('/').next()?;

        version.chars().next()?.is_ascii_digit().then_some(version)
    }
}

/// A thread (a Codex session).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Thread {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    /// The thread it was forked from.
    pub forked_from_id: Option<String>,
    /// Set for a subagent's thread.
    pub parent_thread_id: Option<String>,
    /// Usually the first user message.
    #[serde(deserialize_with = "nullable")]
    pub preview: String,
    pub ephemeral: bool,
    #[serde(deserialize_with = "nullable")]
    pub model_provider: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    /// Seconds since the epoch.
    #[serde(deserialize_with = "lenient_option")]
    pub created_at: Option<u64>,
    #[serde(deserialize_with = "lenient_option")]
    pub updated_at: Option<u64>,
    #[serde(deserialize_with = "nullable")]
    pub status: ThreadStatus,
    #[serde(deserialize_with = "nullable")]
    pub cwd: String,
    pub cli_version: Option<String>,
    pub git_info: Option<GitInfo>,
    /// The name the user (or `thread/name/set`) gave it.
    pub name: Option<String>,
    /// Empty unless the request asked for them.
    #[serde(deserialize_with = "nullable")]
    pub turns: Vec<Turn>,
}

/// Where the project's repository was when the thread started.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GitInfo {
    pub sha: Option<String>,
    pub branch: Option<String>,
}

/// What a loaded thread is doing.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ThreadStatus {
    #[default]
    NotLoaded,
    Idle,
    /// The thread failed (an unusable model, …).
    SystemError,
    /// A turn runs; the flags say what it waits for (`waitingOnApproval`, `waitingOnUserInput`).
    Active {
        #[serde(default, deserialize_with = "nullable")]
        active_flags: Vec<String>,
    },
    #[serde(other)]
    Other,
}

/// `thread/start`, `thread/resume` and `thread/fork`: the thread and the settings it runs with.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ThreadSession {
    pub thread: Thread,
    #[serde(deserialize_with = "nullable")]
    pub model: String,
    #[serde(deserialize_with = "nullable")]
    pub model_provider: String,
    #[serde(deserialize_with = "nullable")]
    pub cwd: String,
    /// `untrusted`, `on-request`, `never`, or a granular object.
    pub approval_policy: Option<Value>,
    /// The sandbox policy (`{"type": "workspaceWrite", …}`).
    pub sandbox: Option<Value>,
    pub reasoning_effort: Option<String>,
    /// The folders the sandbox may write (the project, the user's extra writable roots).
    #[serde(deserialize_with = "nullable")]
    pub runtime_workspace_roots: Vec<String>,
}

/// `turn/start`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct TurnStarted {
    pub turn: Turn,
}

/// `turn/steer`: the turn the input went into.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Steered {
    #[serde(deserialize_with = "nullable")]
    pub turn_id: String,
}

/// `thread/read`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct ThreadRead {
    pub thread: Thread,
}

/// A page of `thread/list`, `thread/turns/list`, `thread/items/list`, `model/list` or
/// `mcpServerStatus/list`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", bound(deserialize = "T: Deserialize<'de>"))]
pub struct Page<T> {
    #[serde(default = "Vec::new", deserialize_with = "nullable")]
    pub data: Vec<T>,
    /// The cursor of the next page; `None` on the last.
    #[serde(default)]
    pub next_cursor: Option<String>,
    /// The cursor for the other direction, when the page has entries.
    #[serde(default)]
    pub backwards_cursor: Option<String>,
}

/// An item of `thread/items/list`, with its turn.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemEntry {
    #[serde(default, deserialize_with = "nullable")]
    pub turn_id: String,
    pub item: ThreadItem,
}

/// A model of `model/list`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Model {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    /// The id `turn/start` takes.
    #[serde(deserialize_with = "nullable")]
    pub model: String,
    #[serde(deserialize_with = "nullable")]
    pub display_name: String,
    #[serde(deserialize_with = "nullable")]
    pub description: String,
    pub hidden: bool,
    #[serde(deserialize_with = "nullable")]
    pub supported_reasoning_efforts: Vec<ReasoningEffortOption>,
    pub default_reasoning_effort: Option<String>,
    /// `text`, `image`.
    #[serde(deserialize_with = "nullable")]
    pub input_modalities: Vec<String>,
    /// The model Codex uses when none is configured.
    pub is_default: bool,
}

/// A reasoning effort a model supports.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReasoningEffortOption {
    #[serde(deserialize_with = "nullable")]
    pub reasoning_effort: String,
    #[serde(deserialize_with = "nullable")]
    pub description: String,
}

/// `skills/list`: the skills per working directory.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct SkillsList {
    #[serde(deserialize_with = "nullable")]
    pub data: Vec<SkillsEntry>,
}

/// The skills of one working directory.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct SkillsEntry {
    #[serde(deserialize_with = "nullable")]
    pub cwd: String,
    #[serde(deserialize_with = "nullable")]
    pub skills: Vec<Skill>,
}

/// A skill.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Skill {
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    #[serde(deserialize_with = "nullable")]
    pub description: String,
    /// Its `SKILL.md`.
    #[serde(deserialize_with = "nullable")]
    pub path: String,
    /// `user`, `repo`, `system` or `admin`.
    #[serde(deserialize_with = "nullable")]
    pub scope: String,
    pub enabled: bool,
    pub plugin_id: Option<String>,
}

/// An MCP server of `mcpServerStatus/list`; its tools by name only (their schemas are large).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpServerStatus {
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    #[serde(deserialize_with = "nullable")]
    pub tools: BTreeMap<String, IgnoredAny>,
    /// Why no tools could be listed.
    pub tools_error: Option<String>,
    /// `unknown`, `unsupported`, `notLoggedIn`, `bearerToken`, `oAuth`.
    pub auth_status: Option<String>,
}

/// `account/read`: whether there is a login and which kind (never its email).
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AccountRead {
    pub account: Option<Account>,
    /// True when Codex needs an OpenAI login (or an API key) to run.
    pub requires_openai_auth: bool,
}

/// The kind of login.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Account {
    ApiKey,
    Chatgpt {
        #[serde(default)]
        plan_type: Option<String>,
    },
    AmazonBedrock,
    #[serde(other)]
    Other,
}
