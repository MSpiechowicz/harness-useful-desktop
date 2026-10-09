import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { RangeTarget, ThreadComment } from '$lib/git/comment-range';
import type { FileDiff } from '$lib/ipc/bindings/FileDiff';

import { binaryFile, modifiedFile } from '../../../tests/diff-samples';
import DiffView from './DiffView.svelte';

/** The sample modified file: two hunks, a removed line, additions and a "no newline" marker. */
const LIB = (): FileDiff => modifiedFile();

function mount(over: { diff?: FileDiff; mode?: 'unified' | 'split'; comments?: ThreadComment[] } = {}) {
  const handlers = { oncomment: vi.fn<(target: RangeTarget, body: string) => void>(), onedit: vi.fn(), onremove: vi.fn() };

  render(DiffView, { diff: over.diff ?? LIB(), mode: over.mode ?? 'unified', comments: over.comments ?? [], maxBytes: 8192, ...handlers });

  return handlers;
}

const gutter = (side: 'old' | 'new', line: number) => screen.getByRole('button', { name: `Comment on ${side} line ${line}` });
const body = () => screen.getByPlaceholderText('Leave a comment for the agent…');

async function commentOn(text: string): Promise<void> {
  await fireEvent.input(body(), { target: { value: text } });
  await fireEvent.click(screen.getByRole('button', { name: 'Comment' }));
}

describe('rendering a diff', () => {
  it('numbers every line on its own side and marks the missing newline', () => {
    mount();

    expect(gutter('new', 3)).toHaveTextContent('3');
    expect(gutter('old', 3)).toHaveTextContent('3');
    expect(gutter('old', 20)).toBeInTheDocument();
    expect(gutter('new', 24)).toBeInTheDocument();
    expect(screen.getByText('No newline at end of file')).toBeInTheDocument();
  });

  it('labels added and removed lines so color is not the only cue', () => {
    mount();

    const removed = document.querySelector('[data-kind="remove"]');

    expect(removed).toHaveTextContent('Removed:');
    expect(document.querySelector('[data-kind="add"]')).toHaveTextContent('Added:');
  });

  it('shows hunk headers as text', () => {
    const diff = LIB();
    mount({ diff });

    expect(screen.getByText(diff.hunks[0].header)).toBeInTheDocument();
    expect(screen.getByText(diff.hunks[1].header)).toBeInTheDocument();
  });

  it('never interprets diff text as markup', () => {
    const diff = LIB();
    diff.hunks[0].lines[0].text = '<img src=x onerror=alert(1)> &amp; <script>boom()</script>';

    mount({ diff });

    expect(document.querySelector('img')).toBeNull();
    expect(document.querySelector('script')).toBeNull();
    expect(screen.getByText(/<img src=x onerror=alert\(1\)> &amp; <script>boom\(\)<\/script>/)).toBeInTheDocument();
  });

  it('says so for a binary file instead of showing hunks', () => {
    mount({ diff: binaryFile() });

    expect(screen.getByRole('status')).toHaveTextContent('Binary file');
    expect(screen.queryByRole('list')).toBeNull();
  });

  it('flags a diff that was cut at a size limit, and one withheld entirely', () => {
    const cut = { ...LIB(), truncated: true };
    const view = render(DiffView, { diff: cut, mode: 'unified', comments: [], maxBytes: 8192, oncomment: vi.fn(), onedit: vi.fn(), onremove: vi.fn() });

    expect(screen.getByRole('status')).toHaveTextContent('cut at a size limit');
    expect(gutter('new', 3)).toBeInTheDocument();
    view.unmount();

    mount({ diff: { ...LIB(), hunks: [], truncated: true } });

    expect(screen.getByRole('status')).toHaveTextContent('too large');
  });

  it('explains a file with no content changes', () => {
    mount({ diff: { ...LIB(), hunks: [], additions: 0, deletions: 0 } });

    expect(screen.getByRole('status')).toHaveTextContent('No content changes');
  });

  it('lays out two sides in split mode, facing a replacement with the line it replaces', () => {
    mount({ mode: 'split' });

    const pair = [...document.querySelectorAll('[data-pair]')].find((row) => row.querySelector('[data-side="old"][data-kind="remove"]') && row.querySelector('[data-side="new"][data-kind="add"]'));

    expect(pair).toBeDefined();
    expect(within(pair as HTMLElement).getByRole('button', { name: 'Comment on old line 3' })).toBeInTheDocument();
    expect(within(pair as HTMLElement).getByRole('button', { name: 'Comment on new line 7' })).toBeInTheDocument();
    // An addition with nothing to replace leaves the old side empty.
    expect(document.querySelector('[data-side="old"][data-kind="empty"]')).not.toBeNull();
  });

  it('colors known languages once the highlighter has run', async () => {
    mount({ diff: modifiedFile('config.json') });

    await waitFor(() => expect(document.querySelector('span[style*="color"]')).not.toBeNull());
  });
});

describe('commenting on lines', () => {
  it('comments on one added line with its number, side and text', async () => {
    const { oncomment } = mount();

    await fireEvent.click(gutter('new', 3));
    expect(screen.getByText('Line 3 (new)')).toBeInTheDocument();
    await commentOn('Why a separate module?');

    expect(oncomment).toHaveBeenCalledWith({ side: 'new', startLine: 3, endLine: 3, snippet: ['mod error;'] }, 'Why a separate module?');
    expect(screen.queryByPlaceholderText('Leave a comment for the agent…')).toBeNull();
  });

  it('comments on a removed line through its old number', async () => {
    const { oncomment } = mount();

    await fireEvent.click(gutter('old', 3));
    await commentOn('Public API break.');

    expect(oncomment).toHaveBeenCalledWith({ side: 'old', startLine: 3, endLine: 3, snippet: ['pub fn parse(input: &str) -> Result<Vec<Record>, String> {'] }, 'Public API break.');
  });

  it('extends a selection to a range with shift-click', async () => {
    const { oncomment } = mount();

    await fireEvent.click(gutter('new', 3));
    await fireEvent.click(gutter('new', 5), { shiftKey: true });
    expect(screen.getByText('Lines 3–5 (new)')).toBeInTheDocument();
    await commentOn('Group these.');

    expect(oncomment).toHaveBeenCalledOnce();
    const [target] = oncomment.mock.calls[0];
    expect(target).toMatchObject({ side: 'new', startLine: 3, endLine: 5 });
    expect(target.snippet).toEqual(['mod error;', '', 'pub use error::Error;']);
  });

  it('drags out a range with the pointer', async () => {
    const { oncomment } = mount();

    await fireEvent.pointerDown(gutter('new', 4), { button: 0 });
    await fireEvent.pointerEnter(gutter('new', 5));
    await fireEvent.pointerEnter(gutter('new', 6));
    await fireEvent.pointerUp(gutter('new', 6));
    await commentOn('Dragged.');

    expect(oncomment.mock.calls[0][0]).toMatchObject({ side: 'new', startLine: 4, endLine: 6 });
  });

  it('keeps a drag on the side it started on', async () => {
    const { oncomment } = mount();

    await fireEvent.pointerDown(gutter('new', 4), { button: 0 });
    await fireEvent.pointerEnter(gutter('old', 3));
    await fireEvent.pointerUp(gutter('old', 3));
    await commentOn('Still new.');

    expect(oncomment.mock.calls[0][0]).toMatchObject({ side: 'new', startLine: 4, endLine: 4 });
  });

  it('cancels with Escape and sends nothing', async () => {
    const { oncomment } = mount();

    await fireEvent.click(gutter('new', 3));
    await fireEvent.keyDown(body(), { key: 'Escape' });

    expect(screen.queryByPlaceholderText('Leave a comment for the agent…')).toBeNull();
    expect(oncomment).not.toHaveBeenCalled();
  });

  it('refuses an empty comment and one over the size limit', async () => {
    mount();

    await fireEvent.click(gutter('new', 3));
    const save = screen.getByRole('button', { name: 'Comment' });
    expect(save).toBeDisabled();

    await fireEvent.input(body(), { target: { value: 'x'.repeat(9000) } });

    expect(save).toBeDisabled();
    expect(screen.getByRole('alert')).toHaveTextContent('Too long');
  });

  it('comments on either side in split mode', async () => {
    const { oncomment } = mount({ mode: 'split' });

    await fireEvent.click(gutter('old', 3));
    await commentOn('Left.');
    await fireEvent.click(gutter('new', 7));
    await commentOn('Right.');

    expect(oncomment.mock.calls.map(([target]) => [target.side, target.startLine])).toEqual([
      ['old', 3],
      ['new', 7],
    ]);
  });
});

describe('saved comments', () => {
  const COMMENTS: ThreadComment[] = [
    { id: 1, side: 'new', startLine: 3, endLine: 5, body: 'Check the module name.' },
    { id: 2, side: 'old', startLine: 3, endLine: 3, body: 'Breaking change.' },
    { id: 3, side: 'new', startLine: 999, endLine: 999, body: 'A line this diff does not show.' },
  ];

  it('shows each comment under the last line it covers', () => {
    mount({ comments: COMMENTS });

    expect(screen.getByText('Check the module name.')).toBeInTheDocument();
    expect(screen.getByText('Breaking change.')).toBeInTheDocument();
    expect(screen.getByText('Lines 3–5 (new)')).toBeInTheDocument();
    expect(screen.queryByText('A line this diff does not show.')).toBeNull();

    const thread = screen.getByText('Check the module name.').closest('li');
    expect(thread?.closest('[role="listitem"]')?.querySelector('[data-line]')).toHaveTextContent('pub use error::Error;');
  });

  it('edits and deletes a comment', async () => {
    const { onedit, onremove } = mount({ comments: COMMENTS });
    const thread = screen.getByText('Breaking change.').closest('li') as HTMLElement;

    await fireEvent.click(within(thread).getByRole('button', { name: 'Edit comment' }));
    await fireEvent.input(body(), { target: { value: 'Breaking change: update the changelog.' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(onedit).toHaveBeenCalledWith(2, 'Breaking change: update the changelog.');

    await fireEvent.click(within(screen.getByText('Check the module name.').closest('li') as HTMLElement).getByRole('button', { name: 'Delete comment' }));

    expect(onremove).toHaveBeenCalledWith(1);
  });

  it('shows the same comments in split mode', () => {
    mount({ mode: 'split', comments: COMMENTS });

    expect(screen.getByText('Check the module name.')).toBeInTheDocument();
    expect(screen.getByText('Breaking change.')).toBeInTheDocument();
  });
});
