import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';
import type { CommandSpec } from '$lib/ipc/bindings/CommandSpec';
import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';
import type { MessageRole } from '$lib/ipc/bindings/MessageRole';
import type { NoticeLevel } from '$lib/ipc/bindings/NoticeLevel';
import type { SessionState } from '$lib/ipc/bindings/SessionState';
import type { Subagent } from '$lib/ipc/bindings/Subagent';
import type { TokenUsage } from '$lib/ipc/bindings/TokenUsage';
import type { UiRequest } from '$lib/ipc/bindings/UiRequest';

/** A block of a message as the transcript shows it; tool calls point at `ChatState.toolCalls`. */
export type DisplayBlock =
  | { type: 'text'; text: string }
  | { type: 'thinking'; text: string; redacted: boolean }
  | { type: 'tool'; toolCallId: string }
  | { type: 'image'; mimeType: string }
  | { type: 'other'; kind: string };

/** What the agent reports about a finished message. */
export interface MessageMeta {
  entryId: string | null;
  model: string | null;
  provider: string | null;
  stopReason: string | null;
  errorMessage: string | null;
  usage: TokenUsage | null;
  costUsd: number | null;
  timestamp: number | null;
}

export interface MessageItem {
  kind: 'message';
  key: string;
  /** Creation order within the session; lets the list animate only what arrives while it is open. */
  order: number;
  role: MessageRole;
  blocks: DisplayBlock[];
  streaming: boolean;
  /** While streaming: content index of a delta block to its position in `blocks`. */
  slots: Record<number, number>;
  meta: MessageMeta | null;
}

export interface NoticeItem {
  kind: 'notice';
  key: string;
  order: number;
  level: NoticeLevel;
  message: string;
}

/** Output of a slash command the agent ran. */
export interface OutputItem {
  kind: 'output';
  key: string;
  order: number;
  text: string;
}

/** A `!command` the user ran through `agent_bash`. */
export interface BashItem {
  kind: 'bash';
  key: string;
  order: number;
  command: string;
  output: string;
  exitCode: number | null;
  cancelled: boolean;
  truncated: boolean;
  running: boolean;
}

export type BannerPhase = 'running' | 'done' | 'failed';

/** Compaction and automatic retries, updated in place as they progress. */
export interface BannerItem {
  kind: 'compaction' | 'retry';
  key: string;
  order: number;
  phase: BannerPhase;
  text: string;
}

export interface UrlItem {
  kind: 'url';
  key: string;
  order: number;
  url: string;
  instructions: string | null;
}

/** A tool card for a call no message block refers to (its message already ended). */
export interface ToolItem {
  kind: 'tool';
  key: string;
  order: number;
  toolCallId: string;
}

/**
 * The chat's memory: the saves its agent holds for the user's review, and what was done with earlier ones. Not part of the
 * agent's conversation, so the reducer never makes one: the list pins it after the last turn while there is something to say.
 */
export interface MemoryItem {
  kind: 'memory';
  key: string;
  order: number;
}

export type TranscriptItem = MessageItem | NoticeItem | OutputItem | BashItem | BannerItem | UrlItem | ToolItem | MemoryItem;

export type ToolStatus = 'pending' | 'running' | 'ok' | 'error';

export interface ToolCallState {
  id: string;
  name: string;
  args: JsonValue | null;
  intent: string | null;
  status: ToolStatus;
  /** Latest streamed output while running. */
  partial: string | null;
  result: JsonValue | null;
  resultText: string | null;
  touchedPaths: string[];
}

export interface PromptError {
  message: string;
  retryable: boolean;
}

export interface CrashInfo {
  code: number | null;
  signal: number | null;
  /** The last stderr lines the agent wrote before it went away. */
  stderr: string[];
}

export interface Widget {
  lines: string[];
  placement: string;
}

export type SessionPhase = 'starting' | 'idle' | 'streaming' | 'exited';

export interface ChatState {
  key: string;
  /** Highest `seq` applied; anything at or below it is a duplicate. */
  lastSeq: number;
  /** Number of envelopes that arrived after a gap in `seq`. */
  gaps: number;
  phase: SessionPhase;
  /** How many times the agent settled; stores react to changes (refresh the context meter). */
  settledCount: number;
  /** How many times the agent reported `ready`. */
  readyCount: number;
  items: TranscriptItem[];
  toolCalls: Record<string, ToolCallState>;
  /** Subagents the session's `task` calls spawned, by id (OMP). */
  subagents: Record<string, Subagent>;
  /** Agent UI requests waiting for an answer, oldest first. */
  openRequests: UiRequest[];
  /** `liveSteered`: leading steering entries already handed to the running response; they cannot be withdrawn. */
  queue: { steering: string[]; followUp: string[]; liveSteered: number };
  statuses: Record<string, string>;
  widgets: Record<string, Widget>;
  title: string | null;
  session: SessionState | null;
  commands: CommandSpec[] | null;
  promptError: PromptError | null;
  crash: CrashInfo | null;
  stderrTail: string[];
  /** Text the agent wants in the composer; `nonce` changes on every request so repeats still apply. */
  editorText: { text: string; nonce: number } | null;
  protocolVersion: number | null;
  /** The last thing the user said; what "Retry" sends again. */
  lastUserText: string | null;
  /** Older messages exist behind this cursor. */
  historyCursor: string | null;
  historyTotal: number | null;
  nextOrder: number;
}

export function messageMeta(message: ChatMessage): MessageMeta {
  return {
    entryId: message.id,
    model: message.model,
    provider: message.provider,
    stopReason: message.stopReason,
    errorMessage: message.errorMessage,
    usage: message.usage,
    costUsd: message.costUsd,
    timestamp: message.timestamp,
  };
}
