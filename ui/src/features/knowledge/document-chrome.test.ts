import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { knowledgeReview } from '$lib/stores/knowledgeReview.svelte';
import { knowledgeReviewDraft } from '$lib/stores/knowledgeReviewDraft.svelte';
import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { startMockApp } from '../../../tests/mock-app';
import DocumentHistory from './DocumentHistory.svelte';
import { fakeKnowledge, resetKnowledge, show } from './knowledge-view-support';

beforeAll(async () => {
  await startMockApp(fixtures.recentProjects[0].path);
});

beforeEach(() => {
  resetKnowledge();
  knowledgeReview.reset();
  knowledgeReviewDraft.reset();
  knowledgeViewer.reset();
});

afterEach(() => {
  nav.go('chat');
  toasts.items = [];
  vi.restoreAllMocks();
});

async function open(title: string, row?: number): Promise<HTMLElement> {
  const cell = row === undefined ? await screen.findByText(title) : within(document.querySelector<HTMLElement>(`[data-row="${row}"]`) as HTMLElement).getByText(title);
  await fireEvent.click(cell);

  return screen.findByRole('region', { name: title });
}

const at = (minutes: number): string => new Date(Date.now() - minutes * 60_000).toISOString();

describe('the history rail', () => {
  it('runs between events, and not after the last one', () => {
    render(DocumentHistory, {
      props: {
      events: [
        { kind: 'asked', by: 'Bob Stone', at: at(1), comment: 'Please look at the retries.' },
        { kind: 'changesRequested', by: 'Dan Poe', at: at(2), comment: 'Fix the retries.' },
        { kind: 'drafted', by: 'Ada Lovelace', at: at(3), comment: null },
      ],
      status: 'ready',
      error: null,
      onretry: () => {},
      },
    });

    const items = [...document.querySelectorAll('ol > li')];

    expect(items).toHaveLength(3);
    expect(items.map((item) => item.querySelector('[data-history-rail]') !== null)).toEqual([true, true, false]);
  });

  it('has no rail for a single event', () => {
    render(DocumentHistory, { props: { events: [{ kind: 'drafted', by: 'Ada Lovelace', at: at(3), comment: null }], status: 'ready', error: null, onretry: () => {} } });

    expect(document.querySelector('[data-history-rail]')).toBeNull();
  });
});

describe('renaming in the panel header', () => {
  it('puts the field in place of the title and the line under it, in the same band', async () => {
    fakeKnowledge();
    await show();

    const panel = await open('Glossary', 10);
    const header = panel.querySelector('header') as HTMLElement;

    expect(header.querySelector('.pane-subtext')).not.toBeNull();
    expect(header.classList.contains('pane-header')).toBe(true);

    await fireEvent.click(within(panel).getByRole('button', { name: 'Rename document' }));

    const field = within(header).getByRole('textbox', { name: 'Document title' }) as HTMLInputElement;

    expect(panel.querySelector('header')).toBe(header);
    expect(header.classList.contains('pane-header')).toBe(true);
    expect(header.querySelector('.pane-subtext')).toBeNull();
    expect(header.querySelector('h2')).toBeNull();
    // The caret waits at the end of the title: nothing is selected.
    await waitFor(() => expect(field.selectionStart).toBe(field.value.length));
    expect(field.selectionEnd).toBe(field.value.length);

    await fireEvent.keyDown(field, { key: 'Escape' });

    await waitFor(() => expect(header.querySelector('.pane-subtext')).not.toBeNull());
  });
});

describe('icons on the buttons of the panel', () => {
  const iconed = (button: HTMLElement): boolean => button.querySelector('svg') !== null;

  it('gives Download, Replace file and Delete an icon each', async () => {
    fakeKnowledge();
    await show();

    const panel = await open('Glossary', 10);

    for (const name of [/^Download/, /^Replace file/, /^Delete/]) {
      expect(iconed(within(panel).getByRole('button', { name }))).toBe(true);
    }
  });

  it('keeps the icon and the chevron on the Download menu of a pending update', async () => {
    fakeKnowledge('drafts', false, {}, false, { name: 'Carol Shaw' });
    await show();

    const panel = await open('Billing API');
    const button = await within(panel).findByRole('button', { name: /^Download/ });

    expect(button.querySelectorAll('svg')).toHaveLength(2);
  });

  it('gives every step of the review row an icon', async () => {
    fakeKnowledge('drafts', false, {}, false, { name: 'Carol Shaw' });
    await show();

    const panel = await open('Billing API');

    for (const name of ['Approve', 'Request changes']) {
      expect(iconed(await within(panel).findByRole('button', { name }))).toBe(true);
    }
  });
});
