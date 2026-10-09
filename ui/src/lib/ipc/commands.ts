import type { AgentAction } from './bindings/AgentAction';
import type { AgentEnvelope } from './bindings/AgentEnvelope';
import type { AgentReply } from './bindings/AgentReply';
import type { AgentSessionInfo } from './bindings/AgentSessionInfo';
import type { AgentStartRequest } from './bindings/AgentStartRequest';
import type { AppInfo } from './bindings/AppInfo';
import type { AppPrefs } from './bindings/AppPrefs';
import type { BashOutput } from './bindings/BashOutput';
import type { Branch } from './bindings/Branch';
import type { CommandSpec as CatalogCommand } from './bindings/CommandSpec';
import type { ComposedReview } from './bindings/ComposedReview';
import type { ConfigView } from './bindings/ConfigView';
import type { DiffRequest } from './bindings/DiffRequest';
import type { ExtensionsView } from './bindings/ExtensionsView';
import type { FileDiff } from './bindings/FileDiff';
import type { GitStatus } from './bindings/GitStatus';
import type { HarnessId } from './bindings/HarnessId';
import type { HarnessSummary } from './bindings/HarnessSummary';
import type { ProjectInfo } from './bindings/ProjectInfo';
import type { ReviewRequest } from './bindings/ReviewRequest';
import type { SessionMessagesPage } from './bindings/SessionMessagesPage';
import type { SessionRef } from './bindings/SessionRef';
import type { SessionSummary } from './bindings/SessionSummary';
import type { SessionTree } from './bindings/SessionTree';
import type { UiResponse } from './bindings/UiResponse';
import type { EventSink } from './transport';

/** Phantom entry: carries the argument and result types of one command, no runtime data. */
export interface CommandSpec<Args, Result> {
  readonly args: Args;
  readonly result: Result;
}

function cmd<Args, Result>(): CommandSpec<Args, Result> {
  return {} as CommandSpec<Args, Result>;
}

/**
 * The single typed command map. Keys are the snake_case names of the Rust
 * `#[tauri::command]` functions; `scripts/check-ipc-contract.ts` compares them 1:1.
 */
export const commands = {
  app_info: cmd<undefined, AppInfo>(),
  prefs_get: cmd<undefined, AppPrefs>(),
  prefs_set: cmd<{ prefs: AppPrefs }, AppPrefs>(),
  open_external: cmd<{ url: string }, null>(),
  reveal_path: cmd<{ path: string }, null>(),
  harness_list: cmd<undefined, HarnessSummary[]>(),
  harness_detect: cmd<{ harness: HarnessId }, HarnessSummary>(),
  harness_pick_binary: cmd<{ harness: HarnessId }, HarnessSummary | null>(),
  harness_clear_binary: cmd<{ harness: HarnessId }, HarnessSummary>(),
  config_view: cmd<{ harness: HarnessId; project: string | null }, ConfigView>(),
  extensions_view: cmd<{ harness: HarnessId; project: string | null }, ExtensionsView>(),
  commands_catalog: cmd<{ harness: HarnessId }, CatalogCommand[]>(),
  project_pick: cmd<undefined, ProjectInfo | null>(),
  project_open: cmd<{ path: string }, ProjectInfo>(),
  project_close: cmd<undefined, null>(),
  project_recent: cmd<undefined, ProjectInfo[]>(),
  project_forget: cmd<{ path: string }, ProjectInfo[]>(),
  agent_start: cmd<{ request: AgentStartRequest; onEvent: EventSink<AgentEnvelope> }, AgentSessionInfo>(),
  agent_send: cmd<{ key: string; action: AgentAction }, AgentReply>(),
  agent_ui_respond: cmd<{ key: string; response: UiResponse }, null>(),
  agent_bash: cmd<{ key: string; command: string; confirmed: boolean }, BashOutput>(),
  agent_stop: cmd<{ key: string }, null>(),
  agent_list: cmd<undefined, AgentSessionInfo[]>(),
  sessions_list: cmd<{ harness: HarnessId; project: string }, SessionSummary[]>(),
  session_tree: cmd<{ session: SessionRef; project: string }, SessionTree>(),
  session_messages: cmd<{ session: SessionRef; project: string; cursor: string | null; limit: number }, SessionMessagesPage>(),
  session_files_touched: cmd<{ session: SessionRef; project: string }, string[]>(),
  git_status: cmd<{ project: string }, GitStatus>(),
  git_branches: cmd<{ project: string }, Branch[]>(),
  git_diff: cmd<{ project: string; request: DiffRequest }, FileDiff[]>(),
  review_compose: cmd<{ request: ReviewRequest }, ComposedReview>(),
};

export type CommandName = keyof typeof commands;
export type CommandArgs<K extends CommandName> = (typeof commands)[K]['args'];
export type CommandResult<K extends CommandName> = (typeof commands)[K]['result'];
