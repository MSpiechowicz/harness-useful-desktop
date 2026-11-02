import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import type { EcosystemId } from '$lib/ipc/bindings/EcosystemId';
import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';
import { ecosystemFixtures } from '$lib/ipc/mock/ecosystem-fixtures';
import { prefs } from '$lib/stores/prefs.svelte';
import { updates } from '$lib/stores/updates.svelte';

import { restoreClientWidth, samplePrefs, stubClientWidth } from '../../../tests/helpers';
import EcosystemHero from './EcosystemHero.svelte';
import EcosystemKernel from './EcosystemKernel.svelte';

type Wanted = 'installed' | 'update' | 'missing' | 'unsupported';

const IDS: EcosystemId[] = ['dashboard', 'memory', 'knowledge', 'code', 'skills', 'sidebar'];

/** The six members of the family, each in the state asked for. */
function family(states: Partial<Record<EcosystemId, Wanted>> = {}): EcosystemItem[] {
  const base = ecosystemFixtures.status.items;

  return IDS.map((id) => {
    const state = states[id] ?? 'installed';
    const item = base.find((entry) => entry.id === id) as EcosystemItem;

    return { ...item, installed: state === 'installed' || state === 'update', updateAvailable: state === 'update', supportedOnThisOs: state !== 'unsupported' };
  });
}

const MIXED = { knowledge: 'update', sidebar: 'missing' } as const;

function status(text: string): HTMLElement {
  return screen.getByText(text);
}

function module(id: EcosystemId): HTMLElement {
  return document.querySelector(`[data-module="${id}"]`) as HTMLElement;
}

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'off' });
});

afterEach(async () => {
  updates.state = { kind: 'idle' };
  restoreClientWidth();
  await i18n.load('en');
});

describe('Family diagram', () => {
  it('shows every member as a module in the state the data says', () => {
    render(EcosystemKernel, { items: family(MIXED) });

    expect(module('memory')).toHaveAccessibleName('Harness Memory installed');
    expect(module('memory')).toHaveAttribute('data-state', 'installed');
    expect(module('knowledge')).toHaveAccessibleName('Harness Knowledge installed, update available');
    expect(module('knowledge')).toHaveAttribute('data-state', 'update');
    expect(module('sidebar')).toHaveAccessibleName('Harness Sidebar not installed');
    expect(module('sidebar')).toHaveAttribute('data-state', 'missing');

    expect(within(module('dashboard')).getByText('Dashboard')).toBeInTheDocument();
    expect(within(module('sidebar')).getByText('Sidebar')).toBeInTheDocument();
  });

  it('marks only a module with an update by a badge and a lit mark only on an installed one', () => {
    render(EcosystemKernel, { items: family(MIXED) });

    expect(module('knowledge').querySelector('.badge')).not.toBeNull();
    expect(module('memory').querySelector('.badge')).toBeNull();
    expect(module('sidebar').querySelector('.badge')).toBeNull();

    // An empty socket shows a plus, not the member's mark.
    expect(module('sidebar').querySelector('[role="img"]')).toBeNull();
    expect(module('memory').querySelector('[role="img"]')).not.toBeNull();
  });

  it('names the diagram by its summary and every module by its state, for a screen reader', () => {
    render(EcosystemKernel, { items: family(MIXED) });

    const group = screen.getByRole('group', { name: '5 of 6 modules loaded · 1 update' });

    expect(within(group).getByRole('button', { name: 'Harness Knowledge installed, update available' })).toBeInTheDocument();
    expect(within(group).getByRole('button', { name: 'Harness Sidebar not installed' })).toBeInTheDocument();
  });

  it('reads the members off the data, in the order given, and skips a tool this system cannot run', () => {
    render(EcosystemKernel, { items: family({ sidebar: 'unsupported' }) });

    expect(document.querySelectorAll('[data-module]')).toHaveLength(5);
    expect(module('sidebar')).toBeNull();
    status('5 of 5 modules loaded');
  });

  it('follows the data while it is on screen', async () => {
    const { rerender } = render(EcosystemKernel, { items: family(MIXED) });

    status('5 of 6 modules loaded · 1 update');

    await rerender({ items: family({ sidebar: 'installed' }) });

    status('6 of 6 modules loaded');
    expect(module('knowledge')).toHaveAttribute('data-state', 'installed');
    expect(module('sidebar')).toHaveAttribute('data-state', 'installed');
  });
});

describe('Status line', () => {
  it('counts the loaded modules and the updates', () => {
    render(EcosystemKernel, { items: family({ knowledge: 'update', code: 'update', sidebar: 'missing', dashboard: 'missing' }) });

    status('4 of 6 modules loaded · 2 updates');
  });

  it('leaves the updates out when there are none', () => {
    render(EcosystemKernel, { items: family() });

    status('6 of 6 modules loaded');
  });

  it('counts none', () => {
    render(EcosystemKernel, { items: family({ dashboard: 'missing', memory: 'missing', knowledge: 'missing', code: 'missing', skills: 'missing', sidebar: 'missing' }) });

    status('0 of 6 modules loaded');
  });

  it('puts a lone member in the singular', () => {
    render(EcosystemKernel, { items: family().slice(0, 1) });

    status('1 of 1 module loaded');
  });

  it('follows the plural of the language of the app', async () => {
    await i18n.load('pl');
    const { unmount } = render(EcosystemKernel, { items: family({ knowledge: 'update', sidebar: 'missing' }) });

    status('Załadowano 5 z 6 modułów · 1 aktualizacja');
    unmount();

    render(EcosystemKernel, { items: family({ dashboard: 'update', memory: 'update', knowledge: 'update' }) });
    status('Załadowano 6 z 6 modułów · 3 aktualizacje');
  });

  it('swaps to the module under the pointer and back', async () => {
    render(EcosystemKernel, { items: family(MIXED) });

    await fireEvent.pointerEnter(module('knowledge'));
    status('Harness Knowledge · Update available');
    expect(screen.queryByText('5 of 6 modules loaded · 1 update')).toBeNull();

    await fireEvent.pointerLeave(module('knowledge'));
    status('5 of 6 modules loaded · 1 update');

    await fireEvent.pointerEnter(module('sidebar'));
    status('Harness Sidebar · Not installed');

    await fireEvent.pointerEnter(module('memory'));
    await fireEvent.pointerLeave(module('sidebar'));
    status('Harness Memory · Installed');
  });

  it('swaps to the module that has the keyboard focus and back', async () => {
    render(EcosystemKernel, { items: family(MIXED) });

    module('knowledge').focus();
    await tick();
    status('Harness Knowledge · Update available');

    module('knowledge').blur();
    await tick();
    status('5 of 6 modules loaded · 1 update');
  });

  it('lights the hovered module and its trace and dims the others', async () => {
    render(EcosystemKernel, { items: family(MIXED) });

    expect(document.querySelectorAll('.dim')).toHaveLength(0);

    await fireEvent.pointerEnter(module('memory'));

    expect(module('memory')).toHaveClass('hot');
    expect(module('knowledge')).toHaveClass('dim');
    expect(document.querySelector('[data-trace="memory"] .trace')).toHaveClass('hot');
    expect(document.querySelector('[data-trace="knowledge"]')).toHaveClass('dim');
  });
});

describe('Choosing a module', () => {
  function cardOf(id: EcosystemId): HTMLElement {
    const card = document.createElement('article');
    const title = document.createElement('h3');

    card.tabIndex = -1;
    title.id = `ecosystem-${id}`;
    card.append(title);
    document.body.append(card);
    card.scrollIntoView = vi.fn();

    return card;
  }

  it('brings its card into view and focus', async () => {
    const card = cardOf('memory');
    render(EcosystemKernel, { items: family() });

    await fireEvent.click(module('memory'));

    expect(card.scrollIntoView).toHaveBeenCalledWith(expect.objectContaining({ block: 'center' }));
    expect(card).toHaveFocus();
    card.remove();
  });

  it('is reached by the keyboard like any button, one tab stop for each module', () => {
    render(EcosystemKernel, { items: family() });

    const buttons = screen.getAllByRole('button');

    expect(buttons).toHaveLength(6);
    expect(buttons.every((button) => button.tabIndex === 0)).toBe(true);
  });

  it('does nothing when the card is not on the page', async () => {
    render(EcosystemKernel, { items: family() });

    await expect(fireEvent.click(module('code'))).resolves.toBe(true);
  });
});

describe('Harness Desktop chip', () => {
  const available = { kind: 'available', info: { version: '9.9.9', currentVersion: '0.51.0', date: null, notes: null } } as never;

  it('has no LED while the app knows of no update', () => {
    render(EcosystemKernel, { items: family() });

    expect(document.querySelector('[data-part="desktop-update"]')).toBeNull();
  });

  it('lights an LED while the app has an update and reads it in the line', async () => {
    updates.state = available;
    render(EcosystemKernel, { items: family() });

    const led = document.querySelector('[data-part="desktop-update"]') as HTMLElement;

    expect(led).not.toBeNull();
    expect(screen.getByRole('img', { name: 'Harness Desktop installed, update available' })).toBe(led);

    await fireEvent.pointerEnter(led.parentElement as HTMLElement);
    status('Harness Desktop · Update available');
    // The chip's own hover reads in the line but dims no module.
    expect(document.querySelectorAll('.dim')).toHaveLength(0);

    await fireEvent.pointerLeave(led.parentElement as HTMLElement);
    status('6 of 6 modules loaded');
  });

  it('says nothing about an update once it is installed and waits for a restart', () => {
    updates.state = { kind: 'readyToRelaunch', info: { version: '9.9.9' } } as never;
    render(EcosystemKernel, { items: family() });

    expect(document.querySelector('[data-part="desktop-update"]')).toBeNull();
  });
});

describe('Layout', () => {
  it('flanks the chip with the modules when wide and stacks them under it when narrow', () => {
    const wide = render(EcosystemKernel, { items: family() });
    const columns = (id: EcosystemId) => module(id).style.left;

    expect(screen.getByRole('group')).toHaveAttribute('data-layout', 'wide');
    expect([columns('dashboard'), columns('memory'), columns('knowledge')]).toEqual(['0px', '0px', '0px']);
    expect(columns('code')).not.toBe('0px');
    expect(columns('code')).toBe(columns('skills'));

    wide.unmount();
    render(EcosystemKernel, { items: family(), narrow: true });

    expect(screen.getByRole('group')).toHaveAttribute('data-layout', 'narrow');
    // Two to a row: the first and second share a row, the third starts the next.
    expect(module('dashboard').style.top).toBe(module('memory').style.top);
    expect(module('knowledge').style.top).not.toBe(module('dashboard').style.top);
    expect(columns('dashboard')).toBe(columns('knowledge'));
    expect(columns('dashboard')).not.toBe(columns('memory'));
  });

  it('stacks the modules when the hero gives the stage less room than it needs', async () => {
    stubClientWidth(440);
    const narrow = render(EcosystemHero, { items: family() });

    await vi.waitFor(() => expect(screen.getByRole('group')).toHaveAttribute('data-layout', 'narrow'));
    narrow.unmount();

    stubClientWidth(1000);
    render(EcosystemHero, { items: family() });

    await vi.waitFor(() => expect(screen.getByRole('group')).toHaveAttribute('data-layout', 'wide'));
  });

  it('seats a family of any size without losing a member', () => {
    for (const count of [2, 4, 8, 11]) {
      const items = Array.from({ length: count }, (_, index) => ({ ...family()[index % IDS.length], id: IDS[index % IDS.length] }));
      const { unmount } = render(EcosystemKernel, { items });

      expect(document.querySelectorAll('[data-module]')).toHaveLength(count);
      unmount();
    }
  });
});

describe('Motion', () => {
  const PULSING = family({ knowledge: 'update', sidebar: 'missing' });

  it('runs the data blips, the chip sweep and the update rings at full motion', () => {
    prefs.value = samplePrefs({ motion: 'full' });
    updates.state = { kind: 'available', info: { version: '9.9.9' } } as never;
    render(EcosystemKernel, { items: PULSING });

    // A blip for each of the five loaded modules, none for the empty socket.
    expect(document.querySelectorAll('[data-blip]')).toHaveLength(5);
    expect(document.querySelector('[data-blip="sidebar"]')).toBeNull();
    expect(document.querySelector('.sweep')).not.toBeNull();
    expect(document.querySelectorAll('.pulse')).toHaveLength(2);
    // The modules are simply there: no entrance animation.
    expect(module('memory')).not.toHaveClass('animated');
  });

  it.each(['reduced', 'off'] as const)('runs no animation at %s motion', (level) => {
    prefs.value = samplePrefs({ motion: level });
    updates.state = { kind: 'available', info: { version: '9.9.9' } } as never;
    render(EcosystemKernel, { items: PULSING });

    expect(document.querySelectorAll('[data-blip]')).toHaveLength(0);
    expect(document.querySelector('.sweep')).toBeNull();
    expect(document.querySelectorAll('.pulse')).toHaveLength(0);
    expect(document.querySelector('.animated')).toBeNull();
    // Everything else stays: the state still reads.
    expect(module('knowledge').querySelector('.badge')).not.toBeNull();
    expect(document.querySelector('[data-part="desktop-update"]')).not.toBeNull();
  });
});
