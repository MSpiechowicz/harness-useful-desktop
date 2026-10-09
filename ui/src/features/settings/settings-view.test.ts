import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { scrollMemory } from '$lib/a11y/scroll-memory';
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
});
