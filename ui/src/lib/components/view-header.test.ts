import { render, screen, within } from '@testing-library/svelte';
import { createRawSnippet, tick, unmount } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { HEADER_CONTROLS_SLOT, HEADER_FIXED_SLOT, HEADER_TITLE_SLOT } from '$lib/actions/portal';
import { LEAVING_ATTRIBUTE } from '$lib/design/header-motion';
import { DURATION } from '$lib/design/motion';
import { prefs } from '$lib/stores/prefs.svelte';
import TopBar from '../../features/shell/TopBar.svelte';

import { samplePrefs } from '../../../tests/helpers';
import KeyedHeader from '../../../tests/fixtures/KeyedHeader.svelte';
import ViewHeader from './ViewHeader.svelte';

beforeAll(() => {
  prefs.value = samplePrefs({ motion: 'off' });
});

const button = (label: string) => createRawSnippet(() => ({ render: () => `<button type="button">${label}</button>` }));

const slot = (id: string) => document.getElementById(id) as HTMLElement;

describe('the top bar and the view header that fills it', () => {
  it('puts the title, the controls and the fixed end each in their own place, in focus order', () => {
    render(TopBar);
    render(ViewHeader, { props: { title: 'Sessions', context: '3 sessions', actions: button('Resume'), fixed: button('More actions') } });

    expect(within(slot(HEADER_TITLE_SLOT)).getByRole('heading', { level: 1, name: 'Sessions' })).toBeInTheDocument();
    expect(slot(HEADER_TITLE_SLOT).querySelector('[data-view-header]')).not.toBeNull();
    expect(within(slot(HEADER_CONTROLS_SLOT)).getByRole('button', { name: 'Resume' })).toBeInTheDocument();
    expect(within(slot(HEADER_FIXED_SLOT)).getByRole('button', { name: 'More actions' })).toBeInTheDocument();

    const order = screen.getAllByRole('button').map((node) => node.textContent);
    expect(order).toEqual(['Resume', 'More actions']);
    expect(screen.getByRole('banner').contains(screen.getByRole('heading', { level: 1 }))).toBe(true);
  });

  it('leaves the fixed end empty for a view with nothing to keep there', () => {
    render(TopBar);
    render(ViewHeader, { props: { title: 'Ecosystem', actions: button('Run setup') } });

    expect(slot(HEADER_FIXED_SLOT).querySelectorAll('button')).toHaveLength(0);
    expect(within(slot(HEADER_CONTROLS_SLOT)).getByRole('button', { name: 'Run setup' })).toBeInTheDocument();
  });

  it('keeps the outgoing fixed end ahead of the incoming one, so the slot can show it until it is gone', () => {
    render(TopBar);
    const outgoing = render(ViewHeader, { props: { title: 'Config', fixed: button('Config menu') } });
    render(ViewHeader, { props: { title: 'Extensions', fixed: button('Extensions menu') } });

    const kept = [...slot(HEADER_FIXED_SLOT).children].map((part) => part.textContent);
    expect(kept).toEqual(['Config menu', 'Extensions menu']);

    outgoing.unmount();

    expect([...slot(HEADER_FIXED_SLOT).children].map((part) => part.textContent)).toEqual(['Extensions menu']);
    expect(slot(HEADER_TITLE_SLOT).querySelectorAll('h1')).toHaveLength(1);
  });

  it('still renders where there is no top bar, so a view can be shown on its own', () => {
    render(ViewHeader, { props: { title: 'Hooks', actions: button('New hook'), fixed: button('More actions') } });

    expect(screen.getByRole('heading', { level: 1, name: 'Hooks' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'New hook' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'More actions' })).toBeInTheDocument();
  });
});

describe('a header that is leaving the top bar', () => {
  afterEach(() => {
    vi.useRealTimers();
    prefs.value = samplePrefs({ motion: 'off' });
  });

  it('has its fixed end marked and inert at once, so it cannot be clicked while the next view takes over', async () => {
    prefs.value = samplePrefs({ motion: 'full' });
    render(TopBar);
    const outgoing = render(ViewHeader, { props: { title: 'Config', fixed: button('Config menu') } });
    const [leaving] = [...slot(HEADER_FIXED_SLOT).children];
    expect(leaving).not.toHaveAttribute(LEAVING_ATTRIBUTE);
    expect(leaving).not.toHaveAttribute('inert');

    const gone = unmount(outgoing.component, { outro: true });

    expect(leaving).toHaveAttribute(LEAVING_ATTRIBUTE);
    expect(leaving).toHaveProperty('inert', true);
    await gone;
    expect(slot(HEADER_FIXED_SLOT).children).toHaveLength(0);
  });
});

describe('a header that comes back while it is still leaving', () => {
  afterEach(() => {
    prefs.value = samplePrefs({ motion: 'off' });
  });

  const titleOf = (name: string) => within(slot(HEADER_TITLE_SLOT)).getByRole('heading', { level: 1, name }).closest('[data-view-header]') as HTMLElement;

  async function backAndForth() {
    prefs.value = samplePrefs({ motion: 'full' });
    render(TopBar);
    const view = render(KeyedHeader, { props: { which: 'Config' } });
    const [fixed] = [...slot(HEADER_FIXED_SLOT).children];
    const title = titleOf('Config');

    await view.rerender({ which: 'Sessions' });
    expect(fixed).toHaveAttribute(LEAVING_ATTRIBUTE);
    await view.rerender({ which: 'Config' });
    await tick();

    return { view, fixed, title };
  }

  it('is no longer marked as leaving, so the next switch treats it as the view on screen', async () => {
    const { fixed, title } = await backAndForth();

    expect(fixed).not.toHaveAttribute(LEAVING_ATTRIBUTE);
    expect(fixed).toHaveProperty('inert', false);
    expect(title).not.toHaveAttribute(LEAVING_ATTRIBUTE);
  });

  it('is marked again when it leaves a second time', async () => {
    const { view, fixed, title } = await backAndForth();

    await view.rerender({ which: 'Sessions' });
    await tick();

    expect(fixed).toHaveAttribute(LEAVING_ATTRIBUTE);
    expect(title).toHaveAttribute(LEAVING_ATTRIBUTE);
  });

  it('counts as the replacement of the title that was leaving meanwhile, so that one fades at once instead of after the patience', async () => {
    const { title } = await backAndForth();
    const outgoing = [...slot(HEADER_TITLE_SLOT).children].find((part) => part !== title) as HTMLElement;

    await vi.waitFor(() => expect(Number(outgoing.style.opacity || 1)).toBeLessThan(1), { timeout: DURATION.slow - 100 });
  });
});
