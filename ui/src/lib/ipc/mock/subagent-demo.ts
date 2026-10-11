import type { AgentEvent } from '../bindings/AgentEvent';
import type { ChatMessage } from '../bindings/ChatMessage';
import type { Subagent } from '../bindings/Subagent';

/** How long the mock waits before each subagent update, so the live card can be watched. */
export const SUBAGENT_STEP_MS = 650;

const CALL_ID = 'call-task';

function message(role: ChatMessage['role'], blocks: ChatMessage['blocks']): ChatMessage {
  return {
    id: null, role, blocks, model: role === 'assistant' ? 'claude-sonnet-4-5' : null, provider: role === 'assistant' ? 'anthropic' : null,
    stopReason: null, errorMessage: null, usage: null, costUsd: role === 'assistant' ? 0.0041 : null, timestamp: Date.now(),
    toolCallId: null, toolName: null, isError: null,
  };
}

const SCOUTS = [
  { id: 'ParserScout', agent: 'explore', task: 'Map every caller of `parse` and how each one handles a missing config.' },
  { id: 'FixtureAuditor', agent: 'explore', task: 'List the test fixtures that feed `parse` and flag the ones with empty input.' },
  { id: 'DocsReviewer', agent: 'reviewer', task: 'Review README and docs/config.md for statements about empty config files.' },
] as const;

function snapshot(index: number, status: Subagent['status'], extra: Partial<Subagent> = {}): Subagent {
  const scout = SCOUTS[index];

  return {
    id: scout.id, parentToolCallId: CALL_ID, agent: scout.agent, status, index, detached: true, description: null, task: scout.task,
    intent: null, currentTool: null, currentArgs: null, recentTools: null, recentOutput: null, toolCount: null, tokens: null,
    contextTokens: null, contextWindow: 200_000, costUsd: null, durationMs: null, model: 'anthropic/claude-haiku-4-5', ...extra,
  };
}

/** One progress step of scout `index`, `step` updates into its run. */
function progress(index: number, step: number, tool: string, args: string, intent: string, output: string[]): AgentEvent {
  const tools = [
    { tool: 'grep', args: 'fn parse', isError: false },
    { tool: 'read', args: 'src/lib.rs', isError: false },
    { tool: 'read', args: 'ui/app.ts', isError: false },
    { tool: 'glob', args: 'tests/**/*.json', isError: false },
  ].slice(0, step);

  return {
    type: 'subagent',
    subagent: snapshot(index, 'running', {
      intent, currentTool: tool, currentArgs: args, recentTools: tools, recentOutput: output, toolCount: step + 1,
      tokens: 3200 * (step + 1) + index * 900, contextTokens: 2900 * (step + 1), costUsd: 0.0011 * (step + 1), durationMs: SUBAGENT_STEP_MS * (step + 2) * (index + 2),
    }),
  };
}

function done(index: number, status: Subagent['status'], tokens: number, durationMs: number): AgentEvent {
  return { type: 'subagent', subagent: snapshot(index, status, { tokens, durationMs, costUsd: tokens * 0.0000009, toolCount: 6 }) };
}

/** A prompt answered by spawning three background scouts (OMP's async `task`), which report back after the turn ends. */
export function subagentDemo(prompt: string): AgentEvent[] {
  const args = { context: 'Empty config input', tasks: SCOUTS.map((scout) => ({ name: scout.id, agent: scout.agent, task: scout.task })) };

  return [
    { type: 'agentStart' },
    { type: 'turnStart' },
    { type: 'messageStart', messageId: 'u', role: 'user' },
    { type: 'messageEnd', messageId: 'u', message: message('user', [{ type: 'text', text: prompt }]) },
    { type: 'messageStart', messageId: 'a1', role: 'assistant' },
    { type: 'textDelta', messageId: 'a1', contentIndex: 0, delta: "I'll split this across three scouts so they can look in parallel." },
    {
      type: 'messageEnd', messageId: 'a1',
      message: message('assistant', [
        { type: 'text', text: "I'll split this across three scouts so they can look in parallel." },
        { type: 'toolCall', id: CALL_ID, name: 'task', args, intent: 'Spawn three scouts' },
      ]),
    },
    { type: 'toolStart', toolCallId: CALL_ID, name: 'task', args, intent: 'Spawn three scouts' },
    ...SCOUTS.map((_, index): AgentEvent => ({ type: 'subagent', subagent: snapshot(index, 'pending') })),
    { type: 'toolEnd', toolCallId: CALL_ID, name: 'task', isError: false, result: { content: [{ type: 'text', text: 'Spawned 3 agents. Results auto-deliver.' }] }, touchedPaths: [] },
    { type: 'messageStart', messageId: 'a2', role: 'assistant' },
    { type: 'textDelta', messageId: 'a2', contentIndex: 0, delta: "They're running in the background; I'll summarize when they report back." },
    { type: 'messageEnd', messageId: 'a2', message: message('assistant', [{ type: 'text', text: "They're running in the background; I'll summarize when they report back." }]) },
    { type: 'turnEnd' },
    { type: 'agentEnd' },
    { type: 'settled', aborted: null },
    progress(0, 0, 'grep', 'fn parse', 'Finding parse callers', ['src/lib.rs:12: pub fn parse(input: &str)']),
    progress(1, 0, 'glob', 'tests/**/*.json', 'Collecting fixtures', []),
    progress(0, 1, 'read', 'src/lib.rs', 'Reading the parser', ['src/lib.rs:12: pub fn parse(input: &str)', 'ui/app.ts:40: const config = parse(text)']),
    progress(2, 0, 'read', 'README.md', 'Reading the README', ['README.md:88: An empty file is an error.']),
    progress(1, 1, 'read', 'tests/fixtures/empty.json', 'Checking the empty fixture', ['tests/fixtures/empty.json: 0 bytes']),
    progress(0, 2, 'read', 'ui/app.ts', 'Checking the UI caller', ['ui/app.ts:40: const config = parse(text)', 'ui/app.ts:41: if (!config) showEmptyState()']),
    done(1, 'completed', 9400, 5200),
    progress(2, 1, 'grep', 'empty', 'Searching docs for "empty"', ['README.md:88: An empty file is an error.', 'docs/config.md:14: Empty files are rejected.']),
    progress(0, 3, 'grep', 'parse(', 'Looking for more callers', ['src/cli.rs:22: let config = parse(&raw)?']),
    done(2, 'failed', 7100, 6900),
    done(0, 'completed', 15_800, 9100),
  ];
}
