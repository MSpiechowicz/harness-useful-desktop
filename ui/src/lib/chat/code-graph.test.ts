import { describe, expect, it } from 'vitest';

import { codeReplyFiles, readCodeGraph } from './code-graph';
import { blankToolCall } from './tools';

// Real replies of harness-code on this repository, cut down: a change of its format should fail here.
const MAP = [
  'centred on chat message: ui/src/lib/chat/tools.ts, ui/src/lib/chat/use-index.ts',
  'ui/src/lib/chat/use-index.ts',
  '  UseAdapter: tool(name: string): string | null',
  '  function sameSet<T>(a: ReadonlySet<T>, b: ReadonlySet<T>): boolean',
  'ui/src/features/chat/ToolCard.svelte',
  '  <ToolCard>',
  '(2 copies of other files left out, such as ui/a.ts)',
  '',
  'Next: code_symbol for where a name above is defined and who uses it, code_impact before changing it.',
].join('\n');

const IMPACT = [
  '`ChatRow`: 12 production file(s), 3 test file(s) within 2 caller hop(s). Entries read `caller → what it calls`',
  '  d0 ui/src/features/chat/ChatRow.svelte  [defined here: ChatRow]',
  '  d1 ui/src/features/chat/ToolCard.svelte  [ToolCard → ChatRow]',
  '  d2 ui/src/features/chat/UserMessage.svelte',
  '  tests: ui/src/features/changes/changes.test.ts (1), ui/src/features/chat/hook-card.test.ts (1), … 1 more',
].join('\n');

const PATH = [
  '`AssistantMessage` → `ChatRow`: 2 hops',
  'from: `AssistantMessage` matches 2 definitions, used: AssistantMessage ui/a.svelte:1, AssistantMessage business/b.rs:155',
  '    AssistantMessage  ui/src/features/chat/AssistantMessage.svelte:1',
  '  → ThinkingBlock  ui/src/features/chat/ThinkingBlock.svelte:1',
  '        at ui/src/features/chat/AssistantMessage.svelte:42',
  '  → ChatRow  ui/src/features/chat/ChatRow.svelte:1',
  '        at ui/src/features/chat/ThinkingBlock.svelte:23',
].join('\n');

const SYMBOL = [
  'fn toSegments  ui/src/lib/markdown/render.ts:77-92',
  '  function toSegments(source: string): Segment[]',
  '  uses:',
  '    Segment ui/src/lib/markdown/render.ts:6  at ui/src/lib/markdown/render.ts:77 | export function toSegments',
  '    htmlOf ui/src/lib/other.ts:68  at ui/src/lib/markdown/render.ts:83 | segments.push',
  '  used by 1 site(s) + 2 test(s):',
  '    ui/src/lib/components/Markdown.svelte:15 Markdown | const segments = $derived(toSegments(source));',
  '    tests: ui/src/lib/markdown/render.test.ts (2)',
].join('\n');

describe('codeReplyFiles', () => {
  it('reads the files of a map, not its definitions or notes', () => {
    expect(codeReplyFiles('map', MAP)).toEqual(['ui/src/lib/chat/use-index.ts', 'ui/src/features/chat/ToolCard.svelte']);
  });

  it('reads the files and the tests of an impact', () => {
    expect(codeReplyFiles('impact', IMPACT)).toEqual([
      'ui/src/features/chat/ChatRow.svelte',
      'ui/src/features/chat/ToolCard.svelte',
      'ui/src/features/chat/UserMessage.svelte',
      'ui/src/features/changes/changes.test.ts',
      'ui/src/features/chat/hook-card.test.ts',
    ]);
  });

  it('reads the steps of a path, not its call sites', () => {
    expect(codeReplyFiles('path', PATH)).toEqual([
      'ui/src/features/chat/AssistantMessage.svelte',
      'ui/src/features/chat/ThinkingBlock.svelte',
      'ui/src/features/chat/ChatRow.svelte',
    ]);
  });

  it('reads no file from a call site or from the candidates of an unsure path', () => {
    const reply = ['    start  ui/a.ts:1', '  → next  ui/b.ts:2', '        at ui/a.ts:9 | run  x.y', 'from `pack`: 2 definitions. Closest:', '  pack ui/c.ts:3  (package.json)'].join('\n');

    expect(codeReplyFiles('path', reply)).toEqual(['ui/a.ts', 'ui/b.ts']);
  });

  it('reads where a symbol is defined and used, not what it depends on', () => {
    expect(codeReplyFiles('symbol', SYMBOL)).toEqual(['ui/src/lib/markdown/render.ts', 'ui/src/lib/components/Markdown.svelte', 'ui/src/lib/markdown/render.test.ts']);
  });

  it('finds nothing in a reply of another shape', () => {
    expect(codeReplyFiles('map', 'no indexed files under src\n')).toEqual([]);
    expect(codeReplyFiles('symbol', 'no definition named `x`. Similar: y\n')).toEqual([]);
    expect(codeReplyFiles('map', null)).toEqual([]);
  });

  it('strips hidden characters from a path', () => {
    expect(codeReplyFiles('map', 'ui/a‮.ts\n')).toEqual(['ui/a.ts']);
  });
});

describe('readCodeGraph', () => {
  const call = (id: string, name: string, resultText: string, status: 'ok' | 'error' = 'ok') => ({ ...blankToolCall(id, name), resultText, status });

  it('merges the lookups of every harness naming, and says which found a file', () => {
    const { lookups, files } = readCodeGraph([call('1', 'mcp__harness-code__code_map', MAP), call('2', 'harness-code_code_impact', IMPACT)]);

    expect(lookups).toBe(2);
    expect(files.find((file) => file.path === 'ui/src/features/chat/ToolCard.svelte')?.kinds).toEqual(['map', 'impact']);
  });

  it('ignores failed calls, other tools and look-alike servers', () => {
    const { lookups, files } = readCodeGraph([call('1', 'mcp__harness-code__code_map', MAP, 'error'), call('2', 'mcp__evil_harness_code__code_map', MAP), call('3', 'bash', MAP)]);

    expect(lookups).toBe(0);
    expect(files).toEqual([]);
  });

  it('does not count a lookup that named no file', () => {
    expect(readCodeGraph([call('1', 'mcp__harness-code__code_symbol', 'no definition named `x`')]).lookups).toBe(0);
  });
});
