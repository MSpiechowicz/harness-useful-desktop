import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { ui, UiState } from '$lib/stores/ui-state.svelte';

import { samplePrefs } from '../../../tests/helpers';
import Rail from './Rail.svelte';
import Shell from './Shell.svelte';
import { DOCUMENTED_SHORTCUTS, globalShortcuts, SHORTCUT_AREAS, shortcutGroups } from './shortcuts';
import ViewHost from './ViewHost.svelte';
import { VIEWS } from './views';

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

describe('keyboard shortcuts overlay', () => {
  it('opens on ? and lists every shell shortcut and every documented one by area', async () => {
    render(Shell);

    await fireEvent.keyDown(document.body, { key: '?', shiftKey: true });

    const overlay = await screen.findByRole('dialog', { name: 'Keyboard shortcuts' });
    const groups = within(overlay).getByTestId('shortcut-groups');
    const registered = globalShortcuts({ togglePalette() {}, openShortcuts() {}, goBack() {}, goForward() {}, newChat() {}, jumpToChat() {}, cycleChat() {}, toggleRail() {}, togglePane() {}, goTo() {} });

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
    const groups = shortcutGroups(globalShortcuts({ togglePalette() {}, openShortcuts() {}, goBack() {}, goForward() {}, newChat() {}, jumpToChat() {}, cycleChat() {}, toggleRail() {}, togglePane() {}, goTo() {} }));
    const goTo = groups.find((group) => group.area === 'Go to');

    expect(goTo?.rows.map((row) => row.label)).toEqual(expect.arrayContaining(VIEWS.map((view) => view.label)));
    expect(goTo?.rows.find((row) => row.label === 'Settings')?.combos).toEqual(['mod+7', 'mod+,']);
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
  it.each(VIEWS.map((view) => view.label))('%s starts with the shared view header and its one h1', async (label) => {
    nav.go(VIEWS.find((view) => view.label === label)!.id);
    render(ViewHost);

    const heading = await screen.findByRole('heading', { level: 1 });
    expect(heading.closest('[data-view-header]')).not.toBeNull();
    expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1);
    expect(heading.textContent).toMatch(label === 'Config' ? /Configuration/ : new RegExp(label));
  });
});
