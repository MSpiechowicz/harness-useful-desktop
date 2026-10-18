import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { scrollMemory } from '$lib/a11y/scroll-memory';
import { ipc } from '$lib/ipc/client';
import { prefs } from '$lib/stores/prefs.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { activeSection, SETTINGS_SECTIONS } from './sections';
import SettingsView from './SettingsView.svelte';

beforeAll(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off' });
  Element.prototype.scrollIntoView = vi.fn();
});

describe('activeSection', () => {
  const boxes = [
    { id: 'appearance', top: -300 },
    { id: 'startup', top: -20 },
    { id: 'harnesses', top: 340 },
  ] as const;

  it('is the last section whose top has passed the reading line', () => {
    expect(activeSection(boxes, 120, false)).toBe('startup');
    expect(activeSection(boxes, 400, false)).toBe('harnesses');
  });

  it('falls back to the first section above the page and to the last at its end', () => {
    expect(activeSection([{ id: 'about', top: 80 }], 20, false)).toBe('about');
    expect(activeSection(boxes, 120, true)).toBe('harnesses');
    expect(activeSection([], 120, false)).toBeNull();
  });
});

describe('SettingsView', () => {
  it('lists every section in a nav and marks the one you jump to', async () => {
    render(SettingsView);
    const nav = screen.getByRole('navigation', { name: 'Settings sections' });

    expect(within(nav).getAllByRole('button').map((button) => button.textContent?.trim())).toEqual(SETTINGS_SECTIONS.map((section) => section.label));
    expect(within(nav).getByRole('button', { name: 'Appearance' })).toHaveAttribute('aria-current', 'true');

    await fireEvent.click(within(nav).getByRole('button', { name: 'Privacy & security' }));

    expect(within(nav).getByRole('button', { name: 'Privacy & security' })).toHaveAttribute('aria-current', 'true');
    expect(screen.getByRole('region', { name: 'Privacy & security' })).toBeInTheDocument();
    expect(Element.prototype.scrollIntoView).toHaveBeenCalled();
  });

  it('narrows the nav to sections whose text matches the search', async () => {
    render(SettingsView);
    const nav = screen.getByRole('navigation', { name: 'Settings sections' });

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search settings' }), { target: { value: 'remember recent' } });

    await waitFor(() => expect(within(nav).getAllByRole('button').map((button) => button.textContent?.trim())).toEqual(['Projects']));

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search settings' }), { target: { value: 'zzzz no such' } });

    await waitFor(() => expect(screen.getByText('No setting matches that search.')).toBeInTheDocument());
  });

  it('toggles project memory through its own command and shows the returned value', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    render(SettingsView);
    const memory = screen.getByRole('switch', { name: 'Project memory' });
    expect(memory).toHaveAttribute('aria-checked', 'true');

    await fireEvent.click(memory);

    expect(invoke).toHaveBeenCalledWith('memory_set_enabled', { enabled: false });
    await waitFor(() => expect(screen.getByRole('switch', { name: 'Project memory' })).toHaveAttribute('aria-checked', 'false'));
    expect(prefs.value?.memoryEnabled).toBe(false);
    expect(screen.getByRole('switch', { name: 'Project memory' })).toBeEnabled();
    expect(invoke).not.toHaveBeenCalledWith('prefs_set', expect.anything());

    await fireEvent.click(screen.getByRole('switch', { name: 'Project memory' }));

    expect(invoke).toHaveBeenLastCalledWith('memory_set_enabled', { enabled: true });
    await waitFor(() => expect(prefs.value?.memoryEnabled).toBe(true));
    invoke.mockRestore();
  });

  it('scrolls to the section the command palette asked for', async () => {
    viewState.settingsTarget = 'projects';

    render(SettingsView);

    const nav = screen.getByRole('navigation', { name: 'Settings sections' });
    await waitFor(() => expect(within(nav).getByRole('button', { name: 'Projects' })).toHaveAttribute('aria-current', 'true'));
    expect(viewState.settingsTarget).toBeNull();
  });
});

describe('scrollMemory', () => {
  function scroller(key: string): { node: HTMLElement; destroy: () => void } {
    const node = document.createElement('div');
    document.body.append(node);
    const action = scrollMemory(node, key);

    return { node, destroy: () => (action?.destroy?.(), node.remove()) };
  }

  it('puts a scroller back where the user left it', async () => {
    const first = scroller('test:pane');
    first.node.scrollTop = 240;
    await fireEvent.scroll(first.node);
    first.destroy();

    const again = scroller('test:pane');

    await waitFor(() => expect(again.node.scrollTop).toBe(240));
    again.destroy();
  });

  it('swaps to the position saved for the new key when the key changes', async () => {
    const node = document.createElement('div');
    document.body.append(node);
    const action = scrollMemory(node, 'test:item:a');

    node.scrollTop = 300;
    await fireEvent.scroll(node);

    action?.update?.('test:item:b');
    await waitFor(() => expect(node.scrollTop).toBe(0));
    node.scrollTop = 120;
    await fireEvent.scroll(node);

    action?.update?.('test:item:a');
    await waitFor(() => expect(node.scrollTop).toBe(300));

    action?.update?.('test:item:b');
    await waitFor(() => expect(node.scrollTop).toBe(120));

    action?.update?.('test:item:never-seen');
    await waitFor(() => expect(node.scrollTop).toBe(0));

    action?.destroy?.();
    node.remove();
  });

  it('keeps only the most recently written positions', () => {
    viewState.rememberScroll('test:cap:first', 10);
    for (let i = 0; i < 300; i += 1) {
      viewState.rememberScroll(`test:cap:${i}`, i + 1);
    }

    expect(viewState.scrollOf('test:cap:first')).toBe(0);
    expect(viewState.scrollOf('test:cap:299')).toBe(300);
  });
});
