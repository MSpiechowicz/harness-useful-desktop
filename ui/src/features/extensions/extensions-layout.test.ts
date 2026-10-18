import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { freshBackend } from '../../../tests/ecosystem-backend';
import { restoreClientWidth, samplePrefs, stubClientWidth } from '../../../tests/helpers';
import ExtensionsViewer from './ExtensionsViewer.svelte';

const WIDE = 1700;
const MIDDLE = 1400;
const NARROW = 1000;

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  await harness.load();
});

beforeEach(() => window.localStorage.clear());

afterEach(() => {
  restoreClientWidth();
});

async function show(width: number): Promise<void> {
  stubClientWidth(width);
  freshBackend();
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp' });
  render(ExtensionsViewer);
  await screen.findByRole('button', { name: 'Extensions' });
}

/** Chooses the plugin in the list, wherever the list is. */
async function choosePlugin(): Promise<void> {
  const group = await screen.findByRole('region', { name: /Plugins/ });
  await fireEvent.click(within(group).getByRole('button', { name: /npm-helper/ }));
}

/** Searches for something nothing matches, which leaves nothing chosen. */
async function chooseNothing(): Promise<void> {
  await fireEvent.input(screen.getByRole('searchbox', { name: 'Search extensions and catalogs' }), { target: { value: 'zzzz-no-such-extension' } });
}

const listToggle = () => screen.getByRole('button', { name: 'Extensions' });
const detailsToggle = () => screen.getByRole('button', { name: 'Details' });

describe('a wide Extensions view', () => {
  it('lays out the list, the centre and the details, both toggles pressed, and the facts of the chosen item in the details', async () => {
    await show(WIDE);
    await choosePlugin();

    expect(listToggle()).toHaveAttribute('aria-pressed', 'true');
    expect(detailsToggle()).toHaveAttribute('aria-pressed', 'true');

    const details = screen.getByRole('region', { name: 'Details' });
    expect(within(details).getByText('npm-helper')).toBeInTheDocument();
    expect(within(details).getByText('Status')).toBeInTheDocument();
    // The centre holds the item itself, not its facts.
    expect(screen.getByRole('article')).toHaveAccessibleName('npm-helper');
  });

  it('asks for a choice in the details while nothing is chosen', async () => {
    await show(WIDE);
    await choosePlugin();
    expect(screen.queryByText('Choose an extension to see its facts.')).toBeNull();

    await chooseNothing();

    expect(await within(screen.getByRole('region', { name: 'Details' })).findByText('Choose an extension to see its facts.')).toBeInTheDocument();
  });

  it('hides the details from the toggle and remembers it for the next time the view is shown', async () => {
    await show(WIDE);

    await fireEvent.click(detailsToggle());

    expect(detailsToggle()).toHaveAttribute('aria-pressed', 'false');
    expect(screen.queryByRole('region', { name: 'Details' })).toBeNull();
    expect(window.localStorage.getItem('harness.workspace.extensions.details')).toBe('false');
  });

  it('opens with the details hidden when they were hidden before', async () => {
    window.localStorage.setItem('harness.workspace.extensions.details', 'false');

    await show(WIDE);

    expect(detailsToggle()).toHaveAttribute('aria-pressed', 'false');
    expect(screen.queryByRole('region', { name: 'Details' })).toBeNull();
  });
});

describe('a middle-width Extensions view', () => {
  it('slides the details over from the toggle and Escape puts them away with focus on the toggle, saving nothing', async () => {
    await show(MIDDLE);
    await choosePlugin();
    expect(screen.queryByRole('region', { name: 'Details' })).toBeNull();
    expect(detailsToggle()).toHaveAttribute('aria-pressed', 'false');

    detailsToggle().focus();
    await fireEvent.click(detailsToggle());

    const details = await screen.findByRole('region', { name: 'Details' });
    expect(within(details).getByText('Status')).toBeInTheDocument();
    expect(detailsToggle()).toHaveAttribute('aria-pressed', 'true');

    within(details).getByRole('button', { name: 'Close' }).focus();
    await fireEvent.keyDown(details, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('region', { name: 'Details' })).toBeNull());
    expect(document.activeElement).toBe(detailsToggle());
    expect(window.localStorage.getItem('harness.workspace.extensions.details')).toBeNull();
  });
});

describe('a narrow Extensions view', () => {
  it('keeps the list in a drawer that its toggle opens and Escape puts away with focus on the toggle', async () => {
    await show(NARROW);
    expect(screen.queryByRole('navigation', { name: 'Extensions' })).toBeNull();
    expect(listToggle()).toHaveAttribute('aria-pressed', 'false');

    listToggle().focus();
    await fireEvent.click(listToggle());

    const drawer = await screen.findByRole('dialog', { name: 'Extensions' });
    expect(within(drawer).getByRole('navigation', { name: 'Extensions' })).toBeInTheDocument();
    expect(listToggle()).toHaveAttribute('aria-pressed', 'true');

    await fireEvent.keyDown(drawer, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Extensions' })).toBeNull());
    expect(document.activeElement).toBe(listToggle());
  });
});
