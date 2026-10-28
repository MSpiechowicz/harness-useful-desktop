import type { AgentEvent } from '../bindings/AgentEvent';
import type { ChatMessage } from '../bindings/ChatMessage';
import type { Subagent } from '../bindings/Subagent';
import type { SubagentRoute } from '../bindings/SubagentRoute';
import type { SubagentStep } from '../bindings/SubagentStep';

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

interface Scout {
  id: string;
  agent: string;
  task: string;
  label: string;
  route: SubagentRoute;
  canStop: boolean | null;
}

const SCOUTS: readonly Scout[] = [
  {
    id: 'ParserScout', agent: 'explore', label: 'Map every caller of parse', route: 'direct', canStop: true,
    task: 'Find every caller of `parse` and how each one handles a missing config.\n\nScope:\n- `src/`\n- `ui/app.ts`\n- the tests under `tests/`\n\nReturn a table of caller, `file:line` and what it does when the config is missing. Do not edit files.',
  },
  {
    id: 'FixtureAuditor', agent: 'explore', label: 'Audit the parse fixtures', route: 'direct', canStop: true,
    task: 'List the test fixtures that feed `parse` and flag the ones with empty input.',
  },
  {
    id: 'DocsReviewer', agent: 'reviewer', label: 'Review the docs on empty config', route: 'orchestrator', canStop: null,
    task: 'Review README and docs/config.md for statements about empty config files.',
  },
];

/** The report FixtureAuditor ends with. */
const FIXTURE_REPORT = `Two of the five fixtures are empty.

- \`tests/fixtures/empty.json\` is 0 bytes and expects \`ParseError::Empty\`.
- \`tests/fixtures/blank.json\` holds only whitespace and expects the same error.

The other three hold a valid config. Nothing else feeds \`parse\` with empty input.`;

/** What the Bash step of ParserScout printed: long enough to scroll, with the end the panel tails. */
const BASH_OUTPUT = Array.from({ length: 36 }, (_, line) => (line % 9 === 8 ? `test parse::case_${line} ... FAILED` : `test parse::case_${line} ... ok`)).join('\n');

const BASE_AT = Date.now();

function snapshot(index: number, status: Subagent['status'], extra: Partial<Subagent> = {}): Subagent {
  const scout = SCOUTS[index];

  return {
    id: scout.id, parentToolCallId: CALL_ID, agent: scout.agent, status, index, detached: true, description: null, task: scout.task,
    intent: null, currentTool: null, currentArgs: null, recentTools: null, recentOutput: null, toolCount: null, tokens: null,
    contextTokens: null, contextWindow: 200_000, costUsd: null, durationMs: null, model: 'anthropic/claude-haiku-4-5',
    label: scout.label, report: null, messageRoute: scout.route, canStop: status === 'pending' ? null : scout.canStop, ...extra,
  };
}

/** A step of scout `index`; `n` orders them in time. */
function step(index: number, n: number, kind: SubagentStep['kind'], name: string | null, detail: string, extra: Partial<SubagentStep> = {}): AgentEvent {
  const entry: SubagentStep = {
    id: `${SCOUTS[index].id}-${n}`, kind, name, detail, status: 'done', at: BASE_AT + n * SUBAGENT_STEP_MS * 4, output: null, outputLines: null, ...extra,
  };

  return { type: 'subagentStep', subagentId: SCOUTS[index].id, step: entry };
}

/** One progress snapshot of scout `index`, `n` steps into its run. */
function progress(index: number, n: number, tool: string, args: string, intent: string, output: string[]): AgentEvent {
  const tools = [
    { tool: 'grep', args: 'fn parse', isError: false },
    { tool: 'read', args: 'src/lib.rs', isError: false },
    { tool: 'read', args: 'ui/app.ts', isError: false },
    { tool: 'glob', args: 'tests/**/*.json', isError: false },
  ].slice(0, n);

  return {
    type: 'subagent',
    subagent: snapshot(index, 'running', {
      intent, currentTool: tool, currentArgs: args, recentTools: tools, recentOutput: output, toolCount: n + 1,
      tokens: 3200 * (n + 1) + index * 900, contextTokens: 2900 * (n + 1), costUsd: 0.0011 * (n + 1), durationMs: SUBAGENT_STEP_MS * (n + 2) * (index + 2),
    }),
  };
}

function done(index: number, status: Subagent['status'], tokens: number, durationMs: number, extra: Partial<Subagent> = {}): AgentEvent {
  return { type: 'subagent', subagent: snapshot(index, status, { tokens, durationMs, costUsd: tokens * 0.0000009, toolCount: 6, ...extra }) };
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
    step(0, 1, 'tool', 'Grep', 'fn parse', { status: 'running' }),
    progress(1, 0, 'glob', 'tests/**/*.json', 'Collecting fixtures', []),
    step(1, 1, 'tool', 'Glob', 'tests/**/*.json', { status: 'running' }),
    step(0, 1, 'tool', 'Grep', 'fn parse'),
    step(0, 2, 'tool', 'Read', 'src/lib.rs', { status: 'running' }),
    progress(0, 1, 'read', 'src/lib.rs', 'Reading the parser', ['src/lib.rs:12: pub fn parse(input: &str)', 'ui/app.ts:40: const config = parse(text)']),
    step(1, 1, 'tool', 'Glob', 'tests/**/*.json'),
    step(1, 2, 'tool', 'Read', 'tests/fixtures/empty.json', { output: 'tests/fixtures/empty.json: 0 bytes', outputLines: 1 }),
    progress(2, 0, 'read', 'README.md', 'Reading the README', ['README.md:88: An empty file is an error.']),
    step(2, 1, 'tool', 'Read', 'README.md', { status: 'running' }),
    step(0, 2, 'tool', 'Read', 'src/lib.rs'),
    step(0, 3, 'text', null, 'The parser takes a &str and returns ParseError::Empty for an empty file. Checking the UI caller next.'),
    progress(1, 1, 'read', 'tests/fixtures/empty.json', 'Checking the empty fixture', ['tests/fixtures/empty.json: 0 bytes']),
    step(0, 4, 'tool', 'Read', 'ui/app.ts', { status: 'running' }),
    progress(0, 2, 'read', 'ui/app.ts', 'Checking the UI caller', ['ui/app.ts:40: const config = parse(text)', 'ui/app.ts:41: if (!config) showEmptyState()']),
    step(0, 4, 'tool', 'Read', 'ui/app.ts'),
    step(0, 5, 'tool', 'Bash', 'cargo test parse:: -- --nocapture', { status: 'running' }),
    step(1, 3, 'tool', 'Grep', 'blank', { status: 'running' }),
    done(1, 'completed', 9400, 5200, { report: FIXTURE_REPORT }),
    step(1, 3, 'tool', 'Grep', 'blank'),
    step(2, 1, 'tool', 'Read', 'README.md'),
    step(2, 2, 'tool', 'Grep', 'empty', { status: 'running' }),
    progress(2, 1, 'grep', 'empty', 'Searching docs for "empty"', ['README.md:88: An empty file is an error.', 'docs/config.md:14: Empty files are rejected.']),
    step(0, 5, 'tool', 'Bash', 'cargo test parse:: -- --nocapture', { output: BASH_OUTPUT, outputLines: 212 }),
    step(0, 6, 'tool', 'Grep', 'parse(', { status: 'running' }),
    progress(0, 3, 'grep', 'parse(', 'Looking for more callers', ['src/cli.rs:22: let config = parse(&raw)?']),
    step(2, 2, 'tool', 'Grep', 'empty', { status: 'failed', output: 'docs/config.md: permission denied', outputLines: 1 }),
    done(2, 'failed', 7100, 6900),
  ];
}
