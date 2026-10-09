import type { AgentEnvelope } from '../bindings/AgentEnvelope';
import type { AgentReply } from '../bindings/AgentReply';
import type { AgentSessionInfo } from '../bindings/AgentSessionInfo';
import type { Branch } from '../bindings/Branch';
import type { CommandSpec } from '../bindings/CommandSpec';
import type { ConfigView } from '../bindings/ConfigView';
import type { ExtensionsView } from '../bindings/ExtensionsView';
import type { FileDiff } from '../bindings/FileDiff';
import type { GitStatus } from '../bindings/GitStatus';
import type { HarnessId } from '../bindings/HarnessId';
import type { HarnessSummary } from '../bindings/HarnessSummary';
import type { ProjectInfo } from '../bindings/ProjectInfo';
import type { SessionMessagesPage } from '../bindings/SessionMessagesPage';
import type { SessionSummary } from '../bindings/SessionSummary';
import type { SessionTree } from '../bindings/SessionTree';
import history from './fixtures/agent/history.json';
import models from './fixtures/agent/models.json';
import sessionInfoOmp from './fixtures/agent/session_info.omp.json';
import sessionInfoPi from './fixtures/agent/session_info.pi.json';
import state from './fixtures/agent/state.json';
import stats from './fixtures/agent/stats.json';
import streamApproval from './fixtures/agent/stream_approval.json';
import streamApprovalDenied from './fixtures/agent/stream_approval_denied.json';
import streamBasic from './fixtures/agent/stream_basic.json';
import streamCrash from './fixtures/agent/stream_crash.json';
import streamError from './fixtures/agent/stream_error.json';
import streamUiRequests from './fixtures/agent/stream_ui_requests.json';
import commandsOmp from './fixtures/commands_catalog.omp.json';
import commandsPi from './fixtures/commands_catalog.pi.json';
import configOmp from './fixtures/config_view.omp.json';
import configPi from './fixtures/config_view.pi.json';
import extensionsOmp from './fixtures/extensions_view.omp.json';
import extensionsPi from './fixtures/extensions_view.pi.json';
import gitBranches from './fixtures/git/git_branches.json';
import gitDiff from './fixtures/git/git_diff.json';
import gitStatus from './fixtures/git/git_status.json';
import harnessList from './fixtures/harness_list.json';
import projectRecent from './fixtures/project_recent.json';
import sessionFilesTouched from './fixtures/sessions/session_files_touched.omp.json';
import sessionMessages from './fixtures/sessions/session_messages.omp.json';
import sessionsListOmp from './fixtures/sessions/sessions_list.omp.json';
import sessionsListPi from './fixtures/sessions/sessions_list.pi.json';
import sessionTree from './fixtures/sessions/session_tree.omp.json';

/**
 * Backend-generated sample data (see `business`); the mock transport serves these
 * untouched so the browser preview shows what the real adapters produce.
 * The JSON module types are widened (`string` instead of literal unions), hence the casts.
 */
export const fixtures = {
  harnesses: harnessList as unknown as HarnessSummary[],
  recentProjects: projectRecent as unknown as ProjectInfo[],
  config: { omp: configOmp, pi: configPi } as unknown as Record<HarnessId, ConfigView>,
  extensions: { omp: extensionsOmp, pi: extensionsPi } as unknown as Record<HarnessId, ExtensionsView>,
  commands: { omp: commandsOmp, pi: commandsPi } as unknown as Record<HarnessId, CommandSpec[]>,
  agent: {
    sessionInfo: { omp: sessionInfoOmp, pi: sessionInfoPi } as unknown as Record<HarnessId, AgentSessionInfo>,
    /** Recorded event streams, one per scenario; `approvalDenied` replays `approval` from the Deny on. */
    streams: {
      basic: streamBasic,
      approval: streamApproval,
      approvalDenied: streamApprovalDenied,
      error: streamError,
      crash: streamCrash,
      uiRequests: streamUiRequests,
    } as unknown as Record<StreamName, AgentEnvelope[]>,
    /** Full `AgentReply` objects. */
    models: models as unknown as Extract<AgentReply, { type: 'models' }>,
    state: state as unknown as Extract<AgentReply, { type: 'state' }>,
    stats: stats as unknown as Extract<AgentReply, { type: 'stats' }>,
    history: history as unknown as Extract<AgentReply, { type: 'history' }>,
  },
  sessions: {
    list: { omp: sessionsListOmp, pi: sessionsListPi } as unknown as Record<HarnessId, SessionSummary[]>,
    /** The OMP session tree and transcript stand in for every session of either harness. */
    tree: sessionTree as unknown as SessionTree,
    messages: sessionMessages as unknown as SessionMessagesPage,
    filesTouched: sessionFilesTouched as string[],
  },
  git: {
    status: gitStatus as unknown as GitStatus,
    branches: gitBranches as unknown as Branch[],
    diff: gitDiff as unknown as FileDiff[],
  },
};

export type StreamName = 'basic' | 'approval' | 'approvalDenied' | 'error' | 'crash' | 'uiRequests';
