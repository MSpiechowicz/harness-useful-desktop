import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { KnowledgeFeatures } from '$lib/ipc/bindings/KnowledgeFeatures';
import type { KnowledgeStatus } from '$lib/ipc/bindings/KnowledgeStatus';
import type { KnowledgeTeamApproval } from '$lib/ipc/bindings/KnowledgeTeamApproval';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { knowledgeStats } from '$lib/stores/knowledgeStats.svelte';
import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { approvalFocus, showTeamApproval } from './approval-focus.svelte';
import { teamApproval } from './team-approval.svelte';
import TeamApprovalRow from './TeamApprovalRow.svelte';

const PULSE = { documents: 3, generation: 1, held: 0, offline: false };

beforeAll(() => {
  Element.prototype.scrollIntoView = vi.fn();
});

beforeEach(() => {
  teamApproval.reset();
  knowledgeStats.reset();
  knowledgeViewer.reset();
  toasts.items = [];
  knowledgeChanges.pulse = { ...PULSE, approval: false };
  knowledgeStatus.value = null;
  approvalFocus.take();
});

afterEach(() => {
  vi.restoreAllMocks();
});

/** Answers every command of the app with what `handler` returns. */
function stub(handler: (command: string, args?: unknown) => unknown) {
  return vi.spyOn(ipc, 'invoke').mockImplementation(handler as never);
}

/** What the team database answers: its setting, and what a change of it comes to. */
function database(options: { value?: KnowledgeTeamApproval; set?: 'done' | 'readOnly' | 'unreachable' | 'noName'; readFails?: IpcFailure; me?: { name?: string } } = {}) {
  const value = options.value ?? { approval: false, place: 'remote', writable: true };

  return stub(async (command, args) => {
    if (command === 'knowledge_approval_get') {
      if (options.readFails) {
        throw options.readFails;
      }

      return value;
    }

    if (command === 'knowledge_stats') {
      return options.me ? { me: options.me } : {};
    }

    if (command === 'knowledge_approval_set') {
      if (options.set === 'noName') {
        return { type: 'refused', code: 'noName', message: 'set git user.name' };
      }

      if (options.set === 'readOnly') {
        return { type: 'refused', code: 'readOnly', message: 'this role may not change the team database' };
      }

      if (options.set === 'unreachable') {
        throw new IpcFailure({ code: 'unreachable', message: 'harness-knowledge: connection refused' });
      }

      return { type: 'done', value: { ...value, approval: (args as { on: boolean }).on } };
    }

    return null;
  });
}

function features(overrides: Partial<KnowledgeFeatures>): KnowledgeStatus {
  const all = { docs: true, import: true, embedder: true, search: true, recall: true, held: true, mcp: true, routing: true, docMatches: true, offlineFallback: true, docsUseful: true, docsSimilar: true, docsAuthors: true, docsDrafts: true, docsApproval: true };

  return { availability: { type: 'ready' }, features: { ...all, ...overrides } } as unknown as KnowledgeStatus;
}

const SWITCH = { name: 'Team approval' };

describe('the team approval switch', () => {
  it('turns approval on in the team database at once and shows what it answered', async () => {
    const invoke = database();
    render(TeamApprovalRow, { remoteSaved: true });

    const toggle = await screen.findByRole('switch', SWITCH);
    await waitFor(() => expect(toggle).toBeEnabled());
    expect(toggle).toHaveAttribute('aria-checked', 'false');

    await fireEvent.click(toggle);

    await waitFor(() => expect(toggle).toHaveAttribute('aria-checked', 'true'));
    expect(invoke).toHaveBeenCalledWith('knowledge_approval_set', { on: true });
    expect(toasts.items.map((item) => item.title)).toEqual(['Team approval is on']);
  });

  it('is greyed out while a change is saving', async () => {
    let finish: (value: unknown) => void = () => undefined;
    stub((command) => {
      if (command === 'knowledge_approval_get') {
        return Promise.resolve({ approval: false, place: 'remote', writable: true });
      }

      return new Promise((resolve) => (finish = resolve));
    });
    render(TeamApprovalRow, { remoteSaved: true });

    const toggle = await screen.findByRole('switch', SWITCH);
    await waitFor(() => expect(toggle).toBeEnabled());
    await fireEvent.click(toggle);

    await waitFor(() => expect(toggle).toBeDisabled());
    finish({ type: 'done', value: { approval: true, place: 'remote', writable: true } });
    await waitFor(() => expect(toggle).toBeEnabled());
  });

  it('keeps the switch where it was when the server refuses, says why in a toast and from then on in the row', async () => {
    database({ set: 'readOnly' });
    render(TeamApprovalRow, { remoteSaved: true });

    const toggle = await screen.findByRole('switch', SWITCH);
    await waitFor(() => expect(toggle).toBeEnabled());
    await fireEvent.click(toggle);

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0]).toMatchObject({ title: 'Team approval did not change', description: 'this role may not change the team database', tone: 'danger' });
    expect(toggle).toHaveAttribute('aria-checked', 'false');
    expect(toggle).toBeDisabled();
    expect(await screen.findByText('Your role on Remote knowledge can only read.')).toBeInTheDocument();
  });

  it('says in a toast that the database is unreachable when a change cannot be sent', async () => {
    database({ set: 'unreachable' });
    render(TeamApprovalRow, { remoteSaved: true });

    const toggle = await screen.findByRole('switch', SWITCH);
    await waitFor(() => expect(toggle).toBeEnabled());
    await fireEvent.click(toggle);

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0].description).toBe('Remote knowledge is unreachable right now. Try again when it is back.');
    expect(toggle).toHaveAttribute('aria-checked', 'false');
  });

  it('reads the setting again when the database says approval changed', async () => {
    const invoke = database();
    render(TeamApprovalRow, { remoteSaved: true });

    const toggle = await screen.findByRole('switch', SWITCH);
    await waitFor(() => expect(toggle).toBeEnabled());
    const reads = () => invoke.mock.calls.filter(([command]) => command === 'knowledge_approval_get').length;
    const before = reads();

    invoke.mockImplementation((async (command: string) => (command === 'knowledge_approval_get' ? { approval: true, place: 'remote', writable: true } : null)) as never);
    knowledgeChanges.pulse = { ...PULSE, approval: true };

    await waitFor(() => expect(toggle).toHaveAttribute('aria-checked', 'true'));
    expect(reads()).toBe(before + 1);
  });
});

describe('who switched it', () => {
  it('says who turned it on and when, with the exact time in a tooltip', async () => {
    database({ value: { approval: true, place: 'remote', writable: true, changedBy: 'Grace Hopper', changedAt: new Date(Date.now() - 3 * 86_400_000).toISOString() } });
    render(TeamApprovalRow, { remoteSaved: true });

    const line = await screen.findByText(/^Turned on by Grace Hopper, /);

    expect(line.textContent).toMatch(/3 days ago/);
    expect(line).toHaveAttribute('title');
  });

  it('says it was turned off, and says "you" for the user', async () => {
    database({ value: { approval: false, place: 'remote', writable: true, changedBy: 'Ada Lovelace', changedAt: new Date(Date.now() - 3_600_000).toISOString() }, me: { name: 'Ada Lovelace' } });
    knowledgeChanges.pulse = { ...PULSE, approval: false, me: 'Ada Lovelace' };
    render(TeamApprovalRow, { remoteSaved: true });

    expect(await screen.findByText(/^Turned off by you, /)).toBeInTheDocument();
  });

  it('says nothing while the database does not say who', async () => {
    database();
    render(TeamApprovalRow, { remoteSaved: true });

    await waitFor(() => expect(screen.getByRole('switch', SWITCH)).toBeEnabled());
    expect(screen.queryByText(/^Turned o/)).toBeNull();
  });
});

describe('a user with no git name', () => {
  it('is told up front that the switch needs one, when the stats say the user has none', async () => {
    database({ me: {} });
    render(TeamApprovalRow, { remoteSaved: true });

    expect(await screen.findByText('Set your git user.name to change this.')).toBeInTheDocument();
    expect(screen.getByRole('switch', SWITCH)).toBeDisabled();
  });

  it('is told so when the server refuses the change for it', async () => {
    database({ set: 'noName' });
    render(TeamApprovalRow, { remoteSaved: true });

    const toggle = await screen.findByRole('switch', SWITCH);
    await waitFor(() => expect(toggle).toBeEnabled());
    await fireEvent.click(toggle);

    expect(await screen.findByText('Set your git user.name to change this.')).toBeInTheDocument();
    expect(toggle).toHaveAttribute('aria-checked', 'false');
    expect(toggle).toBeDisabled();
  });
});

describe('why the switch is greyed out', () => {
  async function disabledWith(reason: string): Promise<void> {
    expect(await screen.findByText(reason)).toBeInTheDocument();
    expect(screen.getByRole('switch', SWITCH)).toBeDisabled();
  }

  it('says to save the connection while no team database is saved, and asks the database nothing', async () => {
    const invoke = database();
    render(TeamApprovalRow, { remoteSaved: false });

    await disabledWith('Save the connection above first. Team approval is kept in the team database.');
    expect(invoke).not.toHaveBeenCalled();
  });

  it('says the team database cannot be reached while the app runs on this computer’s documents', async () => {
    database();
    knowledgeChanges.pulse = { ...PULSE, offline: true };
    render(TeamApprovalRow, { remoteSaved: true });

    await disabledWith('Remote knowledge cannot be reached right now.');
  });

  it('says the same when the read finds nobody answering', async () => {
    database({ readFails: new IpcFailure({ code: 'unreachable', message: 'no route' }) });
    render(TeamApprovalRow, { remoteSaved: true });

    await disabledWith('Remote knowledge cannot be reached right now.');
  });

  it('says a role that only reads cannot change it', async () => {
    database({ value: { approval: true, place: 'remote', writable: false } });
    render(TeamApprovalRow, { remoteSaved: true });

    await disabledWith('Your role on Remote knowledge can only read.');
  });

  it('says to update Harness Knowledge when it has no team approval', async () => {
    database();
    knowledgeStatus.value = features({ docsApproval: false });
    render(TeamApprovalRow, { remoteSaved: true });

    await disabledWith('This version of Harness Knowledge has no team approval. Update it first.');
  });

  it('says the same when the server itself answers that it predates the setting', async () => {
    database({ readFails: new IpcFailure({ code: 'unsupported', message: 'predates team approval' }) });
    render(TeamApprovalRow, { remoteSaved: true });

    await disabledWith('This version of Harness Knowledge has no team approval. Update it first.');
  });

  it('always carries the note that names come from git, so it guards against mistakes', async () => {
    database();
    render(TeamApprovalRow, { remoteSaved: true });

    expect(await screen.findByText(/guards against mistakes, not against someone pretending to be a teammate/)).toBeInTheDocument();
  });
});

describe('the link from the import dialog', () => {
  /** Where the row is on the page: `top` pixels from the top of the window. */
  function rowAt(top: number): void {
    vi.spyOn(Element.prototype, 'getBoundingClientRect').mockReturnValue({ top, bottom: top + 100, left: 0, right: 100, width: 100, height: 100, x: 0, y: top, toJSON: () => ({}) });
  }

  it('opens Settings, scrolls to a row that is out of view and rings it once it got there', async () => {
    database();
    rowAt(5000);
    const go = vi.spyOn(nav, 'go');
    const scroll = vi.mocked(Element.prototype.scrollIntoView);
    scroll.mockClear();

    showTeamApproval();
    expect(go).toHaveBeenCalledWith('settings');
    expect(approvalFocus.requested).toBe(true);

    const { container } = render(TeamApprovalRow, { remoteSaved: true });

    await waitFor(() => expect(scroll).toHaveBeenCalledTimes(1));
    expect(scroll).toHaveBeenCalledWith(expect.objectContaining({ block: 'center' }));
    expect(approvalFocus.requested).toBe(false);
    expect(container.querySelector('[data-flash]')).toBeNull();

    // The scroll ended and the row is on screen now.
    rowAt(200);
    document.dispatchEvent(new Event('scrollend'));
    await waitFor(() => expect(container.querySelector('[data-flash]')).not.toBeNull());
    expect(scroll).toHaveBeenCalledTimes(1);
  });

  it('scrolls again when the page grew under the first scroll and the row is still out of view', async () => {
    database();
    rowAt(5000);
    const scroll = vi.mocked(Element.prototype.scrollIntoView);
    scroll.mockClear();

    showTeamApproval();
    const { container } = render(TeamApprovalRow, { remoteSaved: true });

    await waitFor(() => expect(scroll).toHaveBeenCalledTimes(1));
    document.dispatchEvent(new Event('scrollend'));
    await waitFor(() => expect(scroll).toHaveBeenCalledTimes(2));

    rowAt(200);
    document.dispatchEvent(new Event('scrollend'));
    await waitFor(() => expect(container.querySelector('[data-flash]')).not.toBeNull());
  });

  it('rings a row that is in view at once, without scrolling', async () => {
    database();
    rowAt(200);
    const scroll = vi.mocked(Element.prototype.scrollIntoView);
    scroll.mockClear();

    showTeamApproval();
    const { container } = render(TeamApprovalRow, { remoteSaved: true });

    await waitFor(() => expect(container.querySelector('[data-flash]')).not.toBeNull());
    expect(scroll).not.toHaveBeenCalled();
  });
});
