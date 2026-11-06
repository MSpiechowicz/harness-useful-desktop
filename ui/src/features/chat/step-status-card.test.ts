// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { showNote, showDocument } = vi.hoisted(() => ({ showNote: vi.fn(), showDocument: vi.fn() }));
vi.mock('../memory/memory-open', () => ({ showNote, showScript: vi.fn() }));
vi.mock('./knowledge-open', () => ({ showDocument }));

import { buildKnowledgeUse } from '$lib/chat/knowledge-use';
import { buildMemoryUse } from '$lib/chat/memory-use';
import { blankToolCall } from '$lib/chat/tools';
import type { DisplayBlock, MessageItem, ToolCallState, TranscriptItem } from '$lib/chat/types';

import AssistantMessage from './AssistantMessage.svelte';

const TABLE = (...rows: string[]) => ['| Step | Status |', '| --- | --- |', ...rows].join('\n');

function message(key: string, role: MessageItem['role'], blocks: DisplayBlock[], streaming = false): MessageItem {
  return { kind: 'message', key, order: 0, role, blocks, streaming, slots: {}, meta: null };
}

function assistant(text: string, streaming = false): MessageItem {
  return message('a2', 'assistant', [{ type: 'text', text }], streaming);
}

function call(id: string, name: string, args: ToolCallState['args'], resultText: string): ToolCallState {
  return { ...blankToolCall(id, name), args, resultText, status: 'ok' };
}

/** Renders the reply: a message of the user, a message with the calls, then the final answer. */
function reply(text: string, calls: ToolCallState[] = [], earlier: TranscriptItem[] = [], others: Record<string, ToolCallState> = {}) {
  const item = assistant(text);
  const toolCalls = { ...others, ...Object.fromEntries(calls.map((one) => [one.id, one])) };
  const items: TranscriptItem[] = [
    ...earlier,
    message('u1', 'user', [{ type: 'text', text: 'hello' }]),
    message('a1', 'assistant', calls.map((one) => ({ type: 'tool', toolCallId: one.id }))),
    item,
  ];
  const chat = { items, toolCalls, phase: 'idle' } as Parameters<typeof buildMemoryUse>[0];

  return render(AssistantMessage, { item, toolCalls, active: false, items, memoryUse: buildMemoryUse(chat), knowledgeUse: buildKnowledgeUse(chat) });
}

const open = () => fireEvent.click(screen.getByRole('button', { name: /Status report/ }));

const NOTE_SEARCH = call(
  'n1',
  'mcp__harness-memory__note_search',
  { query: 'status table' },
  ['#61 [location, repo, 2026-11-05, via "claude-code"] Status table comes from hook texts', '  snippet', '#17 [decision, repo, 2026-10-28, via "claude-code"] Memory usefulness in chat', '  snippet'].join('\n'),
);
const NOTE_RATED = call('n2', 'mcp__harness-memory__note_feedback', { ids: [61], reason: 'found the source' }, 'marked 1 note useful: #61');
const DOC_SEARCH = call(
  'k1',
  'mcp__harness-knowledge__knowledge_search',
  { query: 'hooks format' },
  '<knowledge-doc id="17:853" title="Hook file format" section="Matcher" score="961">\nexcerpt\n</knowledge-doc>',
);
const CODE_MAP = call('c1', 'mcp__harness-code__code_map', { focus: 'src' }, 'src/components/SiteFooter.astro\n  <SiteFooter>\nsrc/styles/global.css\n  .footer\n');

beforeEach(() => {
  showNote.mockReset();
  showDocument.mockReset();
});

describe('the Status report card', () => {
  it('lifts the table out of the answer into a closed card that shows the level', () => {
    const { container } = reply(`${TABLE('| Complexity | full — a new card |')}\n\nDone.`);
    const card = container.querySelector('[data-step-status]') as HTMLElement;

    expect(within(card).getByText('Hard')).toBeTruthy();
    expect(container.querySelector('table')).toBeNull();
    expect(screen.queryByText('a new card')).toBeNull();
    expect(card.nextElementSibling?.textContent).toContain('Done.');
  });

  it('leaves any other table in the answer', () => {
    const { container } = reply(TABLE('| Build | passed |'));

    expect(container.querySelector('[data-step-status]')).toBeNull();
    expect(container.querySelector('table')).not.toBeNull();
  });

  it('never shows the table as raw text while it arrives', () => {
    const item = assistant('| Step | Status |\n| --', true);
    const { container } = render(AssistantMessage, { item, toolCalls: {}, active: true });

    expect(container.textContent).not.toContain('Status');
  });

  it('shows the note a reply checked by title, what it did with it, and opens it', async () => {
    reply(TABLE('| Complexity | small — a fix |', '| Notes | opened #61 |'), [NOTE_SEARCH, NOTE_RATED]);
    await open();

    expect(screen.getByText('1 used of 2 checked')).toBeTruthy();
    expect(screen.getByText('status table')).toBeTruthy();
    expect(screen.queryByText(/#61/)).toBeNull();
    expect(screen.getByText('Useful')).toBeTruthy();
    expect(screen.getByText('“found the source”')).toBeTruthy();

    await fireEvent.click(screen.getByRole('button', { name: 'Status table comes from hook texts' }));
    expect(showNote).toHaveBeenCalledWith(61, 'repo');
  });

  it('shows what was checked when the agent says none fit', async () => {
    reply(TABLE('| Knowledge | none fit |'), [DOC_SEARCH]);
    await open();

    expect(screen.getByText('1 checked, none used')).toBeTruthy();
    expect(screen.getByText('Matcher')).toBeTruthy();
    expect(screen.queryByText('No match')).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Hook file format' }));
    expect(showDocument).toHaveBeenCalledWith('17');
  });

  it('shows the files of the code graph by folder', async () => {
    reply(TABLE('| Code Graph | code_map src |'), [CODE_MAP]);
    await open();

    expect(screen.getByText('1 lookup, 2 files')).toBeTruthy();
    expect(screen.getByText('src/components')).toBeTruthy();
    expect(screen.getByText('SiteFooter.astro')).toBeTruthy();
    expect(screen.getAllByText('Map')).toHaveLength(2);
    expect(screen.queryByText('code_map src')).toBeNull();
  });

  it('counts only the calls of this reply', async () => {
    const before = message('a0', 'assistant', [{ type: 'tool', toolCallId: 'old' }]);
    const old = call('old', 'mcp__harness-code__code_map', { focus: 'x' }, 'old/file.ts\n  sig\n');

    reply(TABLE('| Code Graph | code_map src |'), [CODE_MAP], [message('u0', 'user', []), before], { old });
    await open();

    expect(screen.queryByText('file.ts')).toBeNull();
  });

  it('shows what an OMP agent wrote to a tool address, as a direct call', async () => {
    const written = call('w1', 'write', { path: 'xd://mcp__harness_code_code_map', content: '{"query":"footer"}' }, 'src/components/SiteFooter.astro\n  <SiteFooter>\n');
    const search = call('w2', 'write', { path: 'xd://mcp__harness_memory_note_search', content: '{"query":"footer"}' }, 'no matching notes');

    reply(TABLE('| Notes | none fit |', '| Code Graph | code_map footer styles |'), [written, search]);
    await open();

    expect(screen.getByText('1 lookup, 1 file')).toBeTruthy();
    expect(screen.getByText('SiteFooter.astro')).toBeTruthy();
    expect(screen.getByText('No match')).toBeTruthy();
    expect(screen.queryByText('code_map footer styles')).toBeNull();
  });

  it('shows what an OMP agent ran from a script, as a direct call', async () => {
    const script = {
      ...call(
        'e1',
        'eval',
        { code: 'gather(tool.mcp__harness_code_code_map({"focus":"src"}), tool.mcp__harness_knowledge_knowledge_search({"query":"footer"}))' },
        'RESULT 0:\nsrc/components/SiteFooter.astro\n  <SiteFooter>\n\nRESULT 1:\n<knowledge-doc id="17:842" title="Hook file format" section="Folders" score="500">\nx\n</knowledge-doc>\n',
      ),
    };
    const rated = call('e2', 'eval', { code: 'print(await tool.mcp__harness_knowledge_knowledge_feedback({"ids":["17:842"]}))' }, "{'text': 'marked 1 document useful: 17', 'details': {}}");

    reply(TABLE('| Knowledge | none fit |', '| Code Graph | code_map `src/a.ts` |'), [script, rated]);
    await open();

    expect(screen.getByText('1 lookup, 1 file')).toBeTruthy();
    expect(screen.getByText('SiteFooter.astro')).toBeTruthy();
    expect(screen.getByText('1 used of 1 checked')).toBeTruthy();
    expect(screen.getByText('footer')).toBeTruthy();
    expect(screen.getByText('Useful')).toBeTruthy();
  });

  it('does not call a search of the agent what the app found, when its words are unknown', async () => {
    const script = call('w1', 'write', { path: 'xd://mcp__harness_memory_note_search', content: 'not json' }, NOTE_SEARCH.resultText ?? '');

    reply(TABLE('| Notes | none fit |'), [script]);
    await open();

    expect(screen.getByText('Searched by the agent')).toBeTruthy();
    expect(screen.queryByText('Found for your message')).toBeNull();
  });

  it('falls back on what the agent wrote when no call can be read', async () => {
    reply(TABLE('| Code Graph | code_map `src/a.ts` |', '| Notes | opened #12 |'));
    await open();

    expect(screen.getByText('src/a.ts')).toBeTruthy();
    expect(screen.getByText('opened #12')).toBeTruthy();
  });

  it('gathers what found nothing on one line, and hides a limit that is not close', async () => {
    reply(TABLE('| Limits | none close |', '| Scripts | none fit |', '| Notes | none fit |'));
    await open();

    expect(screen.getByText('No match')).toBeTruthy();
    expect(screen.getByText('Scripts, Notes')).toBeTruthy();
    expect(screen.queryByText('Limits')).toBeNull();
  });

  it('shows the questions an agent asked, and no row when none was open', async () => {
    reply(TABLE('| Questions | asked 2 |', '| Notes | opened #12 |'));
    await open();

    expect(screen.getByText('Questions')).toBeTruthy();
    expect(screen.getByText('asked 2')).toBeTruthy();
  });

  it('hides an open-questions row that says none', async () => {
    reply(TABLE('| Questions | none open |', '| Notes | opened #12 |'));
    await open();

    expect(screen.queryByText('Questions')).toBeNull();
    expect(screen.queryByText('No match')).toBeNull();
  });

  it('survives two rows with one label', async () => {
    reply(TABLE('| Complexity | small |', '| Tests | ran unit |', '| Tests | ran e2e |'));
    await open();

    expect(screen.getByText('ran unit')).toBeTruthy();
    expect(screen.getByText('ran e2e')).toBeTruthy();
  });

  it('shows eight files, then the rest on request', async () => {
    const paths = Array.from({ length: 10 }, (_, index) => `src/f${index}.ts\n  sig`).join('\n');

    reply(TABLE('| Code Graph | code_map src |'), [call('c1', 'mcp__harness-code__code_map', {}, paths)]);
    await open();

    expect(screen.queryByText('f9.ts')).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Show 2 more' }));
    expect(screen.getByText('f9.ts')).toBeTruthy();
  });
});
