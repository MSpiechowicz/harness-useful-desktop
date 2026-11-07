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
mod author;
mod chat;
mod code;
mod command;
mod config;
mod config_value;
mod dashboard;
mod dashboard_args;
mod dropped;
mod ecosystem;
mod extension_action;
mod extensions;
mod git;
mod git_sync;
mod harness;
mod hook;
mod ipc_error;
mod knowledge;
mod knowledge_comment;
mod knowledge_held;
mod knowledge_import;
mod knowledge_list;
mod knowledge_remote;
mod knowledge_review;
mod knowledge_similar;
mod knowledge_stats;
mod memory_embedder;
mod memory_guards;
mod memory_import;
mod memory_notes;
mod memory_notes_copy;
mod memory_remote;
mod memory_repos;
mod memory_requests;
mod memory_save;
mod memory_scripts;
mod prefs;
mod project;
mod review;
mod routine;
mod routine_run;
mod session;
mod setup;
mod subagent;
mod subagent_step;
mod sync;
mod sync_apply;
mod ui_request;
mod update;

pub use agent::{AgentSessionInfo, AgentStartRequest, ApprovalMode, BashOutput, SessionRef};
pub(crate) use agent_action::check_subagent_id;
pub use agent_action::{
    AgentAction, AgentReply, IMAGE_MIME_TYPES, ImageAttachment, MAX_ACTION_IMAGE_BYTES, MAX_ACTION_IMAGES,
    MAX_ACTION_TEXT_BYTES, MAX_AGENT_ID_BYTES, MAX_HISTORY_PAGE, MAX_SUBAGENT_ID_BYTES, StreamingBehavior,
};
pub use agent_event::{AgentEnvelope, AgentEvent};
pub use app::AppInfo;
pub use author::{AuthorCount, AuthorFacet, AuthorFilter, is_clean_author_name};
pub use author::{AuthorRef, MAX_AUTHOR_NAME_CHARS, MAX_AUTHOR_VIA_CHARS, lenient_author, lenient_name, lenient_names};
pub use author::{display_text, offline_reason_text};
pub use chat::{
    AgentOption, ChatBlock, ChatMessage, ContextUsage, MessageRole, ModelOption, PromptStatus, SessionState,
    SessionStats, TokenUsage,
};
pub use code::{CodeBuildEvent, CodeIndexState, CodeSwitchResult};
pub use command::{AgentActionKind, CommandInvocation, CommandSource, CommandSpec, GuiTarget};
pub use config::{
    ConfigFormat, ConfigLayer, ConfigSection, ConfigSource, ConfigView, Notice, NoticeLevel, UnmappedRole,
};
pub use config_value::{ConfigField, ConfigValue, SecretKind};
pub use dashboard::{DashboardArgs, DashboardAvailability, DashboardQuery, DashboardResult, DashboardTool};
pub use dashboard_args::{
    BranchCostArgs, BreakdownArgs, DashboardDimension, DashboardKind, DashboardRange, FailuresArgs,
    MAX_DASHBOARD_ID_CHARS, MAX_DASHBOARD_NAME_CHARS, MAX_DASHBOARD_PATH_CHARS, MAX_DASHBOARD_REF_CHARS,
    MAX_DASHBOARD_ROWS, MAX_DASHBOARD_TIME_CHARS, MAX_DASHBOARD_TIPS, SessionCostArgs, TipsArgs, UsageSummaryArgs,
};
pub use dropped::{AttachedFile, DropEvent, DropSpot, DroppedImage};
pub use ecosystem::{EcosystemId, EcosystemInstalled, EcosystemItem, EcosystemRelease, EcosystemStatus, InstallStep};
pub use extension_action::{CliResult, ConfigEdit, ExtensionAction, InstallScope, MAX_CLI_OUTPUT_BYTES};
pub use extensions::{
    AgentInfo, AgentMode, ExtensionModuleInfo, ExtensionScope, ExtensionsView, MarketplaceEntry, MarketplaceInfo,
    McpServerInfo, McpTransport, PackageInfo, PluginFeature, PluginInfo, PromptTemplateInfo, SkillInfo, ThemeInfo,
};
pub use git::{
    Branch, CommitInfo, DiffLine, DiffLineKind, DiffRequest, DiffScope, FileDiff, GitFileStatus, GitStatus, Hunk,
    MAX_BRANCHES, MAX_FILE_DIFF_BYTES, MAX_TOTAL_DIFF_BYTES, MAX_UNTRACKED_FILE_BYTES, StatusEntry,
};
pub use git_sync::{
    ActionOutcome, BranchPage, BranchRow, BranchStart, BranchSync, ConflictFile, GitActionResult, GitOperation,
    GitOperationKind, MAX_BRANCH_QUERY_BYTES, MAX_BRANCH_SEARCH, MainBranch, MainSource, SwitchRequest, SyncAction,
};
pub use harness::{
    Capabilities, Detection, HarnessId, HarnessInfo, HarnessSummary, InstallCommand, InstallHint, NativeApprovalMode,
    VersionSupport,
};
pub use hook::{
    Hook, HookAction, HookApproval, HookAuthor, HookDraft, HookEvent, HookFamilyEntry, HookFamilyStatus, HookListing,
    HookOrigin, HookRequirement, HookScope, HookSeed, HookStatus, HookView, HooksDeleted, HooksEvent, ShareBlock,
    TeamHooks,
};
pub use ipc_error::{IpcError, IpcErrorCode};
pub use knowledge::{
    KnowledgeAvailability, KnowledgeChunk, KnowledgeDeleted, KnowledgeDetail, KnowledgeDocMatch, KnowledgeDocMatches,
    KnowledgeDocument, KnowledgeFeatures, KnowledgeId, KnowledgeLink, KnowledgeOrigin, KnowledgePage, KnowledgePlace,
    KnowledgeRepoCount, KnowledgeReposPage, KnowledgeSection, KnowledgeStatus, KnowledgeSwitchResult,
};
pub use knowledge_comment::{
    KnowledgeComment, KnowledgeCommentChanged, KnowledgeCommentsDeleted, KnowledgeQuoteKind, KnowledgeQuoteLine,
};
pub use knowledge_held::{
    KnowledgeApproval, KnowledgeDecided, KnowledgeDecision, KnowledgeHeldItem, KnowledgeHeldPage, KnowledgeHeldStatus,
    KnowledgeHit, KnowledgePulse, KnowledgeSearchScope,
};
pub use knowledge_import::{
    KnowledgeEmbedding, KnowledgeFileStatus, KnowledgeImportCounts, KnowledgeImportEvent, KnowledgeImportOptions,
    KnowledgeImportResult, KnowledgeIntake, KnowledgeIntakeEntry, KnowledgeReindexed, KnowledgeReplacement,
    KnowledgeSkipReason, KnowledgeSkipped,
};
pub use knowledge_list::{
    KnowledgeListFilter, KnowledgeScope, KnowledgeSort, KnowledgeStatusFilter, KnowledgeUsefulFilter,
};
pub use knowledge_remote::{
    KnowledgeCapabilities, KnowledgeCheck, KnowledgeIndexState, KnowledgePlaceCheck, KnowledgePlaceChecks,
    KnowledgeProblem, KnowledgeRemote,
};
pub use knowledge_review::{
    KnowledgeChangesRequested, KnowledgeCompare, KnowledgeDocStatus, KnowledgeDownloaded, KnowledgeEventKind,
    KnowledgeHistory, KnowledgeHistoryEvent, KnowledgeLastChange, KnowledgeLineComment, KnowledgeOutcome,
    KnowledgePending, KnowledgeRefusalCode, KnowledgeReviewSubmit, KnowledgeSectionDiff, KnowledgeTeamApproval,
    KnowledgeTransition, KnowledgeTransitioned, KnowledgeUpgraded, KnowledgeVerdict,
};
pub use knowledge_similar::{KnowledgeMatch, KnowledgeMatchReason, KnowledgeSimilar, KnowledgeSimilarEntry};
pub use knowledge_stats::{KnowledgePlaceStats, KnowledgeStats, KnowledgeStatsPlaces, KnowledgeViewer};
pub use memory_embedder::{EmbedderPhase, MemoryEmbedderState, MemorySettings, MemoryVectorSwitchResult};
pub use memory_guards::{
    GlobalKindGuard, HeldItem, HeldKind, HeldSaved, HeldTool, KindGuard, MemoryChatGuards, MemoryChatGuardsSet,
    MemoryChatStart, MemoryGuardKind, MemoryGuards, MemoryGuardsPatch, MemoryHeldEvent,
};
pub use memory_import::{
    NotesImportExisting, NotesImportFailure, NotesImportField, NotesImportFileProblem, NotesImportFormat,
    NotesImportPlan, NotesImportPreview, NotesImportProblem, NotesImportProblemCode, NotesImportProgress,
    NotesImportResult, NotesImportRow,
};
pub use memory_notes::{
    MemoryNotesPage, NoteContent, NoteDetail, NoteKind, NoteKindCount, NoteLangFilter, NoteLink, NoteLinkDirection,
    NoteSort, NoteSummary, NoteUsefulFilter, SortOrder,
};
pub use memory_notes_copy::{
    LocalNoteIds, MemoryCopyKind, NoteIdsWord, NotesCopyAction, NotesCopyCounts, NotesCopyFailure, NotesCopyFilter,
    NotesCopyPreview, NotesCopyResult, NotesCopyRow, NotesCopyRows,
};
pub use memory_remote::{MemoryNotesRouting, MemoryNotesTarget, MemoryRemote, MemoryRemoteKind, MemoryRemoteTest};
pub use memory_repos::{MemoryRepoEntry, MemoryRepos, MemoryReposPage};
pub use memory_requests::{
    ScriptPreviousOrigin, ScriptPreviousVersion, ScriptRequestEvent, ScriptRunRequest, ScriptStdin,
};
pub use memory_save::{
    NoteDraft, NoteSaveOutcome, NoteSaved, ScriptDraft, ScriptLanguage, ScriptSaveOutcome, ScriptSaved,
};
pub use memory_scripts::{
    MemoryDeleteAll, MemoryPlaceCounts, MemoryScopeCounts, MemoryScriptsAvailability, MemoryScriptsPage,
    ScriptApproval, ScriptAuthor, ScriptDetail, ScriptGroup, ScriptGroupCounts, ScriptPlace, ScriptReview,
    ScriptReviewCounts, ScriptScope, ScriptSort, ScriptSummary, ScriptTrustItem,
};
pub use prefs::{
    AppPrefs, BinaryOverride, ChatDefaults, DEFAULT_LANGUAGE, LANGUAGES, LimitBarsPref, LimitGaugePref,
    MainBranchChoice, MotionPref, ThemePref, UpdatePolicy,
};
pub use project::{ProjectInfo, ProjectScope};
pub use review::{
    CommentSide, ComposedReview, MAX_REVIEW_COMMENTS, MAX_REVIEW_PROMPT_BYTES, MAX_REVIEW_SNIPPET_LINES,
    MAX_REVIEW_TEXT_BYTES, ReviewComment, ReviewRequest,
};
pub use routine::{
    Routine, RoutineAction, RoutineCounts, RoutineDraft, RoutineFilter, RoutineListing, RoutineModel, RoutineStatus,
    RoutineView, RoutineWhen, RoutineWhenKind, RoutinesDeleted, SchedulePreview, SessionRoutineTag, TeamRoutines,
    TeamRoutinesRemoved,
};
pub use routine_run::{
    RoutineRun, RoutineRunsPage, RoutineRunsQuery, RoutinesEvent, RunRecord, RunSession, RunStatus, RunTrigger,
};
pub use session::{
    MAX_FIRST_PROMPT_CHARS, MAX_NODE_PREVIEW_CHARS, MAX_SESSION_PAGE, MAX_TREE_NODES, ModelTurns, SessionFolder,
    SessionMessagesPage, SessionNode, SessionNodeKind, SessionSummary, SessionTitleSource, SessionTotals, SessionTree,
    TranscriptStats,
};
pub use setup::{
    DashboardInstallResult, DashboardSetup, HarnessSetup, InstallLogLine, InstallStream, ManualRemoval, McpRefusal,
    McpState, MemorySwitchResult, RecommendedSetup, SkillsMigrationFailure, SkillsMigrationReport, SkillsSetup,
    SkillsUnavailableCode,
};
pub use subagent::{MAX_SUBAGENT_TEXT_CHARS, Subagent, SubagentRoute, SubagentStatus, SubagentTool};
pub use subagent_step::{
    MAX_STEP_DETAIL_CHARS, MAX_STEP_OUTPUT_BYTES, MAX_STEP_OUTPUT_LINES, MAX_STEP_TEXT_CHARS, SubagentStep,
    SubagentStepKind, SubagentStepStatus,
};
pub use sync::{
    SyncBlockReason, SyncExcluded, SyncExcludedReason, SyncExportItem, SyncExportPreview, SyncExportSection,
    SyncExported, SyncImportItem, SyncImportPlan, SyncImportSection, SyncInput, SyncInvalidReason, SyncItemKind,
    SyncItemStatus, SyncMcpPreview, SyncNote, SyncStep, SyncUnsupported,
};
pub use sync_apply::{SyncImportResult, SyncItemOutcome, SyncOutcome, SyncProgress, SyncSkipReason};
pub use ui_request::{
    AskAnswer, AskInput, AskOption, AskQuestion, UiOutcome, UiPurpose, UiRequest, UiRequestKind, UiResponse,
    answers_by_position,
};
pub use update::{MAX_UPDATE_NOTES_BYTES, UpdateInfo, UpdatePhase, UpdateProgress};
