import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import type { ToolCallState } from '$lib/chat/types';
import { i18n } from '$lib/i18n/i18n.svelte';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { git } from '$lib/stores/git.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { review } from '$lib/stores/review.svelte';

import { entrySections } from '$lib/git/entries';
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

const files = () => within(screen.getByTestId('file-list'));

async function showChanges(): Promise<void> {
  render(ChangesView);
  await waitFor(() => expect(screen.getByTestId('file-list')).toBeInTheDocument());
}

describe('Changes view', () => {
  it('lists the changed files under Unstaged, Staged and Untracked headings', async () => {
    await showChanges();

    const rows = ENTRIES.reduce((sum, entry) => sum + entrySections(entry).length, 0);
    expect(files().getAllByRole('option')).toHaveLength(rows);
    for (const name of ['Unstaged', 'Staged', 'Untracked']) {
      expect(screen.getByRole('button', { name: new RegExp(`^${name}`) })).toHaveAttribute('aria-expanded', 'true');
    }
  });

  it('shows rename arrows and folds a heading away', async () => {
    await showChanges();
    const renamed = ENTRIES.find((entry) => entry.origPath !== null);

    expect(files().getByText(`${renamed?.origPath} → ${renamed?.path}`)).toBeInTheDocument();

    const before = files().getAllByRole('option').length;
    await fireEvent.click(screen.getByRole('button', { name: /^Staged/ }));

    expect(screen.getByRole('button', { name: /^Staged/ })).toHaveAttribute('aria-expanded', 'false');
    expect(files().getAllByRole('option')).toHaveLength(before - ENTRIES.filter((entry) => entry.staged !== null).length);

    await fireEvent.click(screen.getByRole('button', { name: /^Staged/ }));
    expect(files().getAllByRole('option')).toHaveLength(before);
  });

  it('searches paths and the contents of the diffs', async () => {
    await showChanges();
    const box = screen.getByRole('searchbox', { name: 'Search files and contents' });

    await fireEvent.input(box, { target: { value: 'guide.md' } });
    await waitFor(() => expect(files().getAllByRole('option')).toHaveLength(1));

    await fireEvent.input(box, { target: { value: 'demo::parse' } });
    await waitFor(() => expect(files().getAllByRole('option').length).toBeGreaterThan(0));
    expect(files().getAllByRole('option').length).toBeLessThan(ENTRIES.length);
    expect(files().getAllByText(/^\d+$/).length).toBeGreaterThan(0);

    await fireEvent.input(box, { target: { value: 'zzz-no-such-text' } });
    expect(await screen.findByText(/Nothing matches/)).toBeInTheDocument();
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
    expect(within(prompt).getByRole('heading', { name: LIB })).toBeInTheDocument();
    expect(prompt).toHaveTextContent('Why is this needed?');
    expect(prompt).toHaveTextContent('Then run the tests.');

    expect(screen.getByRole('radiogroup', { name: 'Send to' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /New session/ })).toHaveAttribute('aria-checked', 'true');

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
    await fireEvent.click(await screen.findByRole('button', { name: /Review · 1 comment$/ }));
    const review_ = await screen.findByRole('dialog', { name: 'Review' });
    await fireEvent.click(within(review_).getByRole('button', { name: 'Discard all' }));

    const confirm = await screen.findByRole('dialog', { name: 'Discard this review?' });
    expect(review.commentsFor(path)).toHaveLength(1);

    await fireEvent.click(within(confirm).getByRole('button', { name: 'Keep comments' }));
    expect(review.commentsFor(path)).toHaveLength(1);

    await fireEvent.click(within(review_).getByRole('button', { name: 'Discard all' }));
    await fireEvent.click(within(await screen.findByRole('dialog', { name: 'Discard this review?' })).getByRole('button', { name: 'Discard' }));

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

describe('the review dialog', () => {
  it('searches file paths and comments, marks the words found and says plainly when nothing matches', async () => {
    const path = fixtures.recentProjects[0].path;
    review.add(path, { path: LIB, side: 'new', startLine: 3, endLine: 3, body: 'Rename this retries counter', snippet: ['x'] });
    review.add(path, { path: 'docs/guide.md', side: 'new', startLine: 1, endLine: 2, body: 'Fix the typo', snippet: ['x', 'y'] });
    await showChanges();

    await fireEvent.click(await screen.findByRole('button', { name: /Review · 2 comments/ }));
    const dialog = await screen.findByRole('dialog', { name: 'Review' });
    const box = within(dialog).getByRole('searchbox', { name: 'Search files and comments' });

    await fireEvent.input(box, { target: { value: 'typo' } });
    expect(within(dialog).getByText('1 comment in 1 file')).toBeInTheDocument();
    expect(dialog.querySelector('mark')).toHaveTextContent('typo');
    expect(within(dialog).queryByText(/retries counter/)).toBeNull();

    await fireEvent.input(box, { target: { value: 'guide' } });
    expect(within(dialog).getByText('Fix the typo')).toBeInTheDocument();

    await fireEvent.input(box, { target: { value: 'zzz' } });
    expect(within(dialog).getByText('Nothing matches “zzz”')).toBeInTheDocument();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Clear search' }));
    expect(within(dialog).getAllByRole('button', { name: /guide\.md|lib\.rs/ })).toHaveLength(2);
  });
});

describe('in Polish', () => {
  it('translates the labels and picks the Polish plural form of the pending comments', async () => {
    const path = fixtures.recentProjects[0].path;
    await i18n.load('pl');

    render(ChangesView);
    expect(await screen.findByRole('button', { name: /^Zindeksowane/ })).toBeInTheDocument();
    expect(screen.getByRole('group', { name: 'Zmienione pliki' })).toBeInTheDocument();

    const add = (count: number): void => {
      for (let index = 0; index < count; index += 1) {
        review.add(path, { path: LIB, side: 'new', startLine: index + 1, endLine: index + 1, body: `Note ${index}`, snippet: ['x'] });
      }
    };
    add(2);
    expect(await screen.findByRole('button', { name: 'Przegląd · 2 komentarze' })).toBeInTheDocument();

    add(3);
    expect(await screen.findByRole('button', { name: 'Przegląd · 5 komentarzy' })).toBeInTheDocument();

  });
});
