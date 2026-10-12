//! IPC data-transfer objects shared with the webview.
//!
//! Every type here derives `serde` (camelCase on the wire) and `ts_rs::TS`; `#[ts(export)]`
//! writes one TypeScript file per type into `ui/src/lib/ipc/bindings` when
//! `cargo test -p harness_business export_bindings` runs. The generated files are never edited
//! by hand.

mod agent;
mod agent_action;
mod agent_event;
mod app;
mod chat;
mod command;
mod config;
mod config_value;
mod dashboard;
mod dashboard_args;
mod ecosystem;
mod extension_action;
mod extensions;
mod git;
mod harness;
mod ipc_error;
mod prefs;
mod project;
mod review;
mod session;
mod subagent;
mod ui_request;
mod update;

pub use agent::{AgentSessionInfo, AgentStartRequest, ApprovalMode, BashOutput, SessionRef};
pub use agent_action::{AgentAction, AgentReply, MAX_ACTION_TEXT_BYTES, MAX_HISTORY_PAGE, StreamingBehavior};
pub use agent_event::{AgentEnvelope, AgentEvent};
pub use app::AppInfo;
pub use chat::{
    ChatBlock, ChatMessage, ContextUsage, MessageRole, ModelOption, PromptStatus, SessionState, SessionStats,
    TokenUsage,
};
pub use command::{AgentActionKind, CommandInvocation, CommandSource, CommandSpec, GuiTarget};
pub use config::{ConfigFormat, ConfigLayer, ConfigSection, ConfigSource, ConfigView, Notice, NoticeLevel};
pub use config_value::{ConfigField, ConfigValue, SecretKind};
pub use dashboard::{DashboardArgs, DashboardAvailability, DashboardQuery, DashboardResult, DashboardTool};
pub use dashboard_args::{
    BranchCostArgs, BreakdownArgs, DashboardDimension, DashboardKind, DashboardRange, FailuresArgs,
    MAX_DASHBOARD_ID_CHARS, MAX_DASHBOARD_NAME_CHARS, MAX_DASHBOARD_PATH_CHARS, MAX_DASHBOARD_REF_CHARS,
    MAX_DASHBOARD_ROWS, MAX_DASHBOARD_TIME_CHARS, MAX_DASHBOARD_TIPS, SessionCostArgs, TipsArgs, UsageSummaryArgs,
};
pub use ecosystem::{EcosystemId, EcosystemInstalled, EcosystemItem, EcosystemRelease, EcosystemStatus, InstallStep};
pub use extension_action::{CliResult, ExtensionAction, InstallScope, MAX_CLI_OUTPUT_BYTES};
pub use extensions::{
    ExtensionModuleInfo, ExtensionScope, ExtensionsView, MarketplaceEntry, MarketplaceInfo, McpServerInfo,
    McpTransport, PackageInfo, PluginFeature, PluginInfo, PromptTemplateInfo, SkillInfo, ThemeInfo,
};
pub use git::{
    Branch, CommitInfo, DiffLine, DiffLineKind, DiffRequest, DiffScope, FileDiff, GitFileStatus, GitStatus, Hunk,
    MAX_BRANCHES, MAX_FILE_DIFF_BYTES, MAX_TOTAL_DIFF_BYTES, MAX_UNTRACKED_FILE_BYTES, StatusEntry,
};
pub use harness::{Capabilities, Detection, HarnessId, HarnessInfo, HarnessSummary, InstallHint, VersionSupport};
pub use ipc_error::{IpcError, IpcErrorCode};
pub use prefs::{
    AppPrefs, BinaryOverride, ChatDefaults, DEFAULT_LANGUAGE, LANGUAGES, MotionPref, ThemePref, UpdatePolicy,
};
pub use project::ProjectInfo;
pub use review::{
    CommentSide, ComposedReview, MAX_REVIEW_COMMENTS, MAX_REVIEW_PROMPT_BYTES, MAX_REVIEW_SNIPPET_LINES,
    MAX_REVIEW_TEXT_BYTES, ReviewComment, ReviewRequest,
};
pub use session::{
    MAX_FIRST_PROMPT_CHARS, MAX_NODE_PREVIEW_CHARS, MAX_SESSION_PAGE, MAX_TREE_NODES, SessionMessagesPage, SessionNode,
    SessionNodeKind, SessionSummary, SessionTitleSource, SessionTree,
};
pub use subagent::{Subagent, SubagentStatus, SubagentTool};
pub use ui_request::{UiOutcome, UiRequest, UiRequestKind, UiResponse};
pub use update::{MAX_UPDATE_NOTES_BYTES, UpdateInfo, UpdatePhase, UpdateProgress};
