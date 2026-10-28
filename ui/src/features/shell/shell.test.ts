import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Subagent } from '$lib/ipc/bindings/Subagent';
import { WorkspaceState } from '$lib/components/workspace.svelte';
import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { subagentPanel } from '$lib/stores/subagentPanel.svelte';
import { ui, UiState } from '$lib/stores/ui-state.svelte';

import { chatSession } from '../../../tests/chat-helpers';
import { samplePrefs } from '../../../tests/helpers';
import Rail from './Rail.svelte';
import Shell from './Shell.svelte';
import { DOCUMENTED_SHORTCUTS, globalShortcuts, SHORTCUT_AREAS, shortcutGroups } from './shortcuts';
import ViewHost from './ViewHost.svelte';
import { VIEWS, viewHint, viewShortcut } from './views';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp', onboardingCompleted: true });
  await harness.load();
  await project.pick();
});

beforeEach(() => {
  window.localStorage.clear();
  nav.go('chat');
  ui.setRailExpanded(false);
  ui.roomForLabels = true;
});

/** `matchMedia` that reports a window wide enough for the expanded rail. */
function wideWindow(): void {
  vi.stubGlobal('matchMedia', (query: string) => ({ matches: query.includes('min-width'), media: query, addEventListener: () => {}, removeEventListener: () => {} }));
}

function subagent(id: string, status: Subagent['status']): Subagent {
  return {
    id, parentToolCallId: null, agent: 'explore', status, index: 0, detached: false, description: null, task: null, intent: null, currentTool: null,
    currentArgs: null, recentTools: null, recentOutput: null, toolCount: null, tokens: null, contextTokens: null, contextWindow: null, costUsd: null,
    durationMs: null, model: null, label: null, report: null, messageRoute: null, canStop: null,
  };
}

describe('keyboard shortcuts overlay', () => {
  it('opens on ? and lists every shell shortcut and every documented one by area', async () => {
    render(Shell);

    await fireEvent.keyDown(document.body, { key: '?', shiftKey: true });

    const overlay = await screen.findByRole('dialog', { name: 'Keyboard shortcuts' });
    const groups = within(overlay).getByTestId('shortcut-groups');
    const registered = globalShortcuts({ togglePalette() {}, openShortcuts() {}, goBack() {}, goForward() {}, newChat() {}, jumpToChat() {}, cycleChat() {}, toggleRail() {}, toggleList() {}, toggleDetails() {}, goTo() {} });

    for (const shortcut of registered) {
      expect(within(groups).getByText(shortcut.label)).toBeInTheDocument();
    }
    for (const doc of DOCUMENTED_SHORTCUTS) {
      expect(within(groups).getAllByText(doc.label).length).toBeGreaterThan(0);
    }
    expect(within(groups).getAllByRole('heading', { level: 3 }).map((heading) => heading.textContent)).toEqual(SHORTCUT_AREAS);

    await fireEvent.keyDown(overlay, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Keyboard shortcuts' })).toBeNull());
  });

  it('goes to Notes, Scripts and Hooks with Ctrl+7, 8 and 9, and Settings keeps only Ctrl+comma', async () => {
    render(Shell);

    await fireEvent.keyDown(document.body, { key: '7', ctrlKey: true });
    expect(nav.current).toBe('notes');

    await fireEvent.keyDown(document.body, { key: '8', ctrlKey: true });
    expect(nav.current).toBe('scripts');

    await fireEvent.keyDown(document.body, { key: '9', ctrlKey: true });
    expect(nav.current).toBe('hooks');

    await fireEvent.keyDown(document.body, { key: ',', ctrlKey: true });
    expect(nav.current).toBe('settings');

    expect(viewShortcut('hooks')).toBe('mod+9');
    expect(viewShortcut('settings')).toBe('mod+,');
    expect(viewHint('settings')).toBe('mod+,');
  });

  it('gives Routines no combo, in the shortcuts, the overlay or the rail', async () => {
    const noop = { togglePalette() {}, openShortcuts() {}, goBack() {}, goForward() {}, newChat() {}, jumpToChat() {}, cycleChat() {}, toggleRail() {}, toggleList() {}, toggleDetails() {}, goTo() {} };
    const registered = globalShortcuts(noop);

    expect(viewShortcut('routines')).toBeNull();
    expect(viewHint('routines')).toBeNull();
    expect(registered.some((shortcut) => shortcut.id === 'go-routines')).toBe(false);
    expect(registered.filter((shortcut) => shortcut.combo === 'mod+,').map((shortcut) => shortcut.id)).toEqual(['go-settings']);
    expect(shortcutGroups(registered).flatMap((group) => group.rows).some((row) => row.label === 'Routines')).toBe(false);

    render(Rail);
    await fireEvent.click(within(screen.getByRole('navigation', { name: 'Primary' })).getByRole('button', { name: 'Routines' }));

    expect(nav.current).toBe('routines');
  });

  it('does not steal a typed ? but still opens with Ctrl+/ from a text field', async () => {
    render(Shell);
    const field = document.createElement('textarea');
    document.body.append(field);
    field.focus();

    await fireEvent.keyDown(field, { key: '?', shiftKey: true });
    expect(screen.queryByRole('dialog', { name: 'Keyboard shortcuts' })).toBeNull();

    await fireEvent.keyDown(field, { key: '/', ctrlKey: true });
    expect(await screen.findByRole('dialog', { name: 'Keyboard shortcuts' })).toBeInTheDocument();
    field.remove();
  });

  it('groups the shell shortcuts under their areas', () => {
    const groups = shortcutGroups(globalShortcuts({ togglePalette() {}, openShortcuts() {}, goBack() {}, goForward() {}, newChat() {}, jumpToChat() {}, cycleChat() {}, toggleRail() {}, toggleList() {}, toggleDetails() {}, goTo() {} }));
    const goTo = groups.find((group) => group.area === 'Go to');

    // Every view with a combo is listed, and Routines, which has none, is not.
    expect(goTo?.rows.map((row) => row.label)).toEqual(expect.arrayContaining(VIEWS.filter((view) => view.id !== 'routines').map((view) => view.label)));
    expect(goTo?.rows.map((row) => row.label)).not.toContain('Routines');
    expect(goTo?.rows.find((row) => row.label === 'Hooks')?.combos).toEqual(['mod+9']);
    // Past the ninth view there is no digit left: Settings keeps only the comma.
    expect(goTo?.rows.find((row) => row.label === 'Settings')?.combos).toEqual(['mod+,']);
  });
});

describe('the list and details shortcuts', () => {
  const noop = { togglePalette() {}, openShortcuts() {}, goBack() {}, goForward() {}, newChat() {}, jumpToChat() {}, cycleChat() {}, toggleRail() {}, goTo() {} };

  it('puts the list on Ctrl+[ (Ctrl+Shift+B still works) and the details on Ctrl+]', () => {
    const registered = globalShortcuts({ ...noop, toggleList() {}, toggleDetails() {} });

    expect(registered.find((shortcut) => shortcut.id === 'toggle-list')).toMatchObject({ combo: 'mod+[', also: ['mod+shift+b'] });
    expect(registered.find((shortcut) => shortcut.id === 'toggle-details')).toMatchObject({ combo: 'mod+]' });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    document.querySelectorAll('[data-test-extra]').forEach((node) => node.remove());
  });

  /** A text field outside the shell, focused, that the test's keys are sent from. */
  function focusedField(): HTMLTextAreaElement {
    const field = document.createElement('textarea');
    field.dataset.testExtra = '';
    document.body.append(field);
    field.focus();

    return field;
  }

  it('reaches the view on screen from the keyboard, from a text field too', async () => {
    const list = vi.spyOn(WorkspaceState, 'toggleCurrentList').mockReturnValue(true);
    const details = vi.spyOn(WorkspaceState, 'toggleCurrentDetails').mockReturnValue(true);
    nav.go('sessions');
    render(Shell);
    const field = focusedField();

    await fireEvent.keyDown(field, { key: '[', ctrlKey: true });
    await fireEvent.keyDown(field, { key: 'B', ctrlKey: true, shiftKey: true });
    await fireEvent.keyDown(field, { key: ']', ctrlKey: true });
    await fireEvent.keyDown(field, { key: '+', code: 'BracketRight', ctrlKey: true });

    expect(list).toHaveBeenCalledTimes(2);
    expect(details).toHaveBeenCalledTimes(1);
  });

  it('opens the subagent panel with Ctrl+] in Chat: the running subagent first, else the newest, and closes it again', async () => {
    const details = vi.spyOn(WorkspaceState, 'toggleCurrentDetails').mockReturnValue(true);
    const { session } = chatSession([
      { type: 'subagent', subagent: subagent('Older', 'completed') },
      { type: 'subagent', subagent: subagent('Runner', 'running') },
      { type: 'subagent', subagent: subagent('Newest', 'completed') },
    ]);
    agentSessions.sessions = [session];
    agentSessions.activeKey = session.key;
    render(Shell);
    const field = focusedField();

    await fireEvent.keyDown(field, { key: ']', ctrlKey: true });
    expect(subagentPanel.selectedId(session.key)).toBe('Runner');

    await fireEvent.keyDown(field, { key: ']', ctrlKey: true });
    expect(subagentPanel.selectedId(session.key)).toBeNull();

    session.apply((state) => ({ ...state, subagents: { Older: state.subagents.Older, Newest: state.subagents.Newest } }));
    await fireEvent.keyDown(field, { key: ']', ctrlKey: true });
    expect(subagentPanel.selectedId(session.key)).toBe('Newest');
    expect(details).not.toHaveBeenCalled();

    subagentPanel.close(session.key);
    agentSessions.sessions = [];
    agentSessions.activeKey = null;
  });

  it('leaves the panels alone, and focus where it is, while a modal dialog is open', async () => {
    const list = vi.spyOn(WorkspaceState, 'toggleCurrentList').mockReturnValue(true);
    const details = vi.spyOn(WorkspaceState, 'toggleCurrentDetails').mockReturnValue(true);
    render(Shell);
    const dialog = document.createElement('div');
    dialog.dataset.testExtra = '';
    dialog.setAttribute('role', 'dialog');
    dialog.setAttribute('aria-modal', 'true');
    const inside = document.createElement('textarea');
    dialog.append(inside);
    document.body.append(dialog);
    inside.focus();

    for (const init of [{ key: '[' }, { key: ']' }, { key: 'B', shiftKey: true }]) {
      await fireEvent.keyDown(inside, { ...init, ctrlKey: true });
    }

    expect(list).not.toHaveBeenCalled();
    expect(details).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(inside);
  });

  it('gives the keys back as soon as a dialog starts closing', async () => {
    const list = vi.spyOn(WorkspaceState, 'toggleCurrentList').mockReturnValue(true);
    render(Shell);
    // A dialog fading out: Svelte makes it inert for the length of its outro.
    const closing = document.createElement('div');
    closing.dataset.testExtra = '';
    closing.setAttribute('inert', '');
    const dialog = document.createElement('div');
    dialog.setAttribute('role', 'dialog');
    dialog.setAttribute('aria-modal', 'true');
    closing.append(dialog);
    document.body.append(closing);

    await fireEvent.keyDown(window, { key: '[', ctrlKey: true });

    expect(list).toHaveBeenCalledTimes(1);
  });
});

describe('keyboard shortcuts behind a confirmation', () => {
  it('leaves the views, history, chats and the rail alone while a confirmation dialog is open, and gives them back once it closes', async () => {
    render(Shell);
    render(ConfirmDialog, { open: true, title: 'Throw away?', description: 'This cannot be undone.', confirmLabel: 'Throw away', onconfirm: () => {} });
    const dialog = await screen.findByRole('dialog', { name: 'Throw away?' });
    const before = nav.current;
    const railBefore = ui.railExpanded;

    await fireEvent.keyDown(dialog, { key: '2', ctrlKey: true });
    await fireEvent.keyDown(dialog, { key: 'ArrowLeft', altKey: true });
    await fireEvent.keyDown(dialog, { key: 'ArrowRight', altKey: true });
    await fireEvent.keyDown(dialog, { key: 'b', ctrlKey: true });

    expect(nav.current).toBe(before);
    expect(ui.railExpanded).toBe(railBefore);

    await fireEvent.keyDown(dialog, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Throw away?' })).toBeNull());
    await fireEvent.keyDown(document.body, { key: 'b', ctrlKey: true });

    expect(ui.railExpanded).toBe(!railBefore);
  });
});

describe('rail', () => {
  it('shows labels once expanded, and remembers the choice across launches', async () => {
    wideWindow();
    render(Rail);
    const rail = screen.getByRole('navigation', { name: 'Primary' });
    expect(rail).toHaveAttribute('data-expanded', 'false');

    await fireEvent.click(screen.getByRole('button', { name: 'Expand sidebar' }));

    expect(rail).toHaveAttribute('data-expanded', 'true');
    expect(within(rail).getByText('Extensions')).toBeVisible();
    expect(screen.getByRole('button', { name: 'Collapse sidebar' })).toHaveAttribute('aria-expanded', 'true');
    expect(new UiState().railExpanded).toBe(true);

    await fireEvent.click(screen.getByRole('button', { name: 'Collapse sidebar' }));

    expect(new UiState().railExpanded).toBe(false);
  });

  it('stays icon-only, with no toggle, when the window is too narrow', async () => {
    vi.stubGlobal('matchMedia', (query: string) => ({ matches: false, media: query, addEventListener: () => {}, removeEventListener: () => {} }));
    ui.setRailExpanded(true);
    render(Rail);

    await waitFor(() => expect(screen.getByRole('navigation', { name: 'Primary' })).toHaveAttribute('data-expanded', 'false'));
    expect(screen.queryByRole('button', { name: 'Collapse sidebar' })).toBeNull();
  });
});

describe('view headers', () => {
  // The host loads these pages with a dynamic import. The first import of each is a cold transform that can take longer
  // than a query waits when many test files run at once, so the pages are loaded here and the host finds them ready.
  beforeAll(async () => {
    await Promise.all([import('../chat/ChatView.svelte'), import('../sessions/SessionsView.svelte'), import('../changes/ChangesView.svelte')]);
  }, 30_000);

  it.each(VIEWS.map((view) => view.label))('%s starts with the shared view header and its one h1', async (label) => {
    nav.go(VIEWS.find((view) => view.label === label)!.id);
    render(ViewHost);

    const heading = await screen.findByRole('heading', { level: 1 });
    expect(heading.closest('[data-view-header]')).not.toBeNull();
    expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1);
    expect(heading.textContent).toMatch(label === 'Config' ? /Configuration/ : new RegExp(label));
  });
});
