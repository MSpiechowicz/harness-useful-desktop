import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import type { ToolCallState } from '$lib/chat/types';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { git } from '$lib/stores/git.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { review } from '$lib/stores/review.svelte';

import ToolCard from '../chat/ToolCard.svelte';
import ChangesView from './ChangesView.svelte';

const ENTRIES = fixtures.git.status.entries;
const LIB = ENTRIES.find((entry) => entry.path.endsWith('.rs') && entry.unstaged === 'modified')?.path ?? ENTRIES[0].path;

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');

  await prefs.load();
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  await project.open(fixtures.recentProjects[0].path);
});

afterEach(() => {
  review.discard(fixtures.recentProjects[0].path);
  git.activate(null);
  nav.go('chat');
});

const files = () => within(screen.getByRole('listbox', { name: 'Changed files' }));

async function showChanges(): Promise<void> {
  render(ChangesView);
  await waitFor(() => expect(screen.getByRole('listbox', { name: 'Changed files' })).toBeInTheDocument());
}

describe('Changes view', () => {
  it('shows the branch, its distance from upstream and every changed file', async () => {
    await showChanges();

    expect(screen.getByText(fixtures.git.status.branch ?? '')).toBeInTheDocument();
    expect(screen.getByText('ahead', { exact: false })).toBeInTheDocument();
    expect(files().getAllByRole('option')).toHaveLength(ENTRIES.length);
    expect(screen.getByRole('radio', { name: /^All/ })).toHaveAttribute('aria-checked', 'true');
  });

  it('narrows the list with the scope tabs and shows rename arrows', async () => {
    await showChanges();
    const renamed = ENTRIES.find((entry) => entry.origPath !== null);

    expect(files().getByText(`${renamed?.origPath} → ${renamed?.path}`)).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('radio', { name: /^Staged/ }));

    const staged = ENTRIES.filter((entry) => entry.staged !== null);
    expect(files().getAllByRole('option')).toHaveLength(staged.length);

    await fireEvent.click(screen.getByRole('radio', { name: /^Untracked/ }));

    expect(files().getAllByRole('option')).toHaveLength(ENTRIES.filter((entry) => entry.unstaged === 'untracked').length);
  });

  it('shows the diff of the chosen file with counts and keyboard navigation', async () => {
    await showChanges();

    await fireEvent.click(files().getByTitle(LIB));

    await waitFor(() => expect(screen.getAllByRole('button', { name: /^Comment on (new|old) line/ }).length).toBeGreaterThan(0));
    expect(screen.getByRole('heading', { name: LIB })).toBeInTheDocument();

    const selected = files().getByRole('option', { selected: true });
    await fireEvent.keyDown(selected, { key: 'ArrowUp' });

    expect(files().getByRole('option', { selected: true })).not.toBe(selected);
  });

  it('keeps only the files a session touched, until the filter is cleared', async () => {
    git.showPaths(fixtures.recentProjects[0].path, [LIB], 'this session');
    expect(nav.current).toBe('changes');

    await showChanges();

    expect(screen.getByTestId('touched-filter')).toHaveTextContent('Touched by this session · 1 file');
    expect(files().getAllByRole('option')).toHaveLength(1);

    await fireEvent.click(screen.getByRole('button', { name: 'Show all changes' }));

    expect(screen.queryByTestId('touched-filter')).toBeNull();
    expect(files().getAllByRole('option')).toHaveLength(ENTRIES.length);
  });

  it('says so when none of the touched files has changes', async () => {
    git.showPaths(fixtures.recentProjects[0].path, ['does/not/exist.rs'], 'this chat');

    render(ChangesView);

    expect(await screen.findByText(/None of the files this session touched/)).toBeInTheDocument();
  });

  it('opens from a tool card with that card\'s touched files', async () => {
    await project.open(fixtures.recentProjects[0].path);
    const call: ToolCallState = { id: 'c1', name: 'edit', args: { path: LIB }, intent: null, status: 'ok', partial: null, result: null, resultText: 'done', touchedPaths: [LIB] };
    render(ToolCard, { call, active: false });

    await fireEvent.click(screen.getByRole('button', { name: /edit/ }));
    await fireEvent.click(screen.getByRole('button', { name: 'View changes' }));

    expect(nav.current).toBe('changes');
    expect(git.filter).toEqual({ label: 'this chat', paths: [LIB] });
  });
});

describe('reviewing changes', () => {
  async function comment(line: number, text: string, shift = false): Promise<void> {
    await fireEvent.click(await screen.findByRole('button', { name: `Comment on new line ${line}` }), { shiftKey: shift });
    await fireEvent.input(screen.getByPlaceholderText('Leave a comment for the agent…'), { target: { value: text } });
    await fireEvent.click(screen.getByRole('button', { name: 'Comment' }));
  }

  /** A modified file whose diff has added lines 3..5, as the sample shows. */
  async function openFirstDiff(): Promise<void> {
    await showChanges();
    await fireEvent.click(files().getByTitle(LIB));
  }

  it('collects comments into a tray, previews the exact prompt and sends it to a new session', async () => {
    const path = fixtures.recentProjects[0].path;
    nav.go('changes');
    await openFirstDiff();
    const first = fixtures.git.diff.find((diff) => diff.path === LIB)?.hunks[0].lines.find((line) => line.kind === 'add');
    const line = first?.newNo ?? 1;

    await comment(line, 'Why is this needed?');

    expect(await screen.findByRole('button', { name: /Review · 1 comment$/ })).toBeInTheDocument();
    expect(files().getByTitle('1 pending comment')).toBeInTheDocument();
    expect(review.commentsFor(path)).toHaveLength(1);

    await fireEvent.click(screen.getByRole('button', { name: /Review · 1 comment$/ }));
    await fireEvent.input(screen.getByLabelText(/Overall instruction/), { target: { value: 'Then run the tests.' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Preview & send' }));

    const prompt = await screen.findByLabelText('Prompt to send');
    expect(prompt).toHaveTextContent(`### ${LIB}`);
    expect(prompt).toHaveTextContent('Why is this needed?');
    expect(prompt).toHaveTextContent('Then run the tests.');

    await fireEvent.click(screen.getByRole('button', { name: 'Send to agent' }));

    await waitFor(() => expect(nav.current).toBe('chat'));
    expect(agentSessions.sessions.length).toBeGreaterThan(0);
    expect(review.commentsFor(path)).toEqual([]);
    expect(review.noteFor(path)).toBe('');
  });

  it('warns before discarding the pending comments', async () => {
    const path = fixtures.recentProjects[0].path;
    await openFirstDiff();
    const added = fixtures.git.diff.find((diff) => diff.path === LIB)?.hunks[0].lines.find((line) => line.kind === 'add');

    await comment(added?.newNo ?? 1, 'Will be discarded.');
    await fireEvent.click(await screen.findByRole('button', { name: 'Discard' }));

    expect(screen.getByRole('dialog', { name: 'Discard this review?' })).toBeInTheDocument();
    expect(review.commentsFor(path)).toHaveLength(1);

    await fireEvent.click(screen.getByRole('button', { name: 'Keep comments' }));
    expect(review.commentsFor(path)).toHaveLength(1);

    await fireEvent.click(screen.getByRole('button', { name: 'Discard' }));
    await fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Discard' }));

    expect(review.commentsFor(path)).toEqual([]);
  });

  it('keeps comments when switching away and back', async () => {
    const path = fixtures.recentProjects[0].path;
    await openFirstDiff();
    const added = fixtures.git.diff.find((diff) => diff.path === LIB)?.hunks[0].lines.find((line) => line.kind === 'add');
    await comment(added?.newNo ?? 1, 'Still here.');

    git.activate(null);
    expect(review.commentsFor(path)).toHaveLength(1);
  });
});
