import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { restoreClientWidth, samplePrefs, stubClientWidth } from '../../../tests/helpers';
import { configMode } from './config-mode.svelte';
import ConfigViewer from './ConfigViewer.svelte';

const WIDE = 1700;
const MIDDLE = 1400;
const NARROW = 1000;

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp' });
  await harness.load();
});

beforeEach(() => window.localStorage.clear());

afterEach(() => {
  restoreClientWidth();
  configMode.set('structured');
  viewState.configQuery = '';
  viewState.configSection = null;
});

async function showConfig(width: number): Promise<void> {
  stubClientWidth(width);
  render(ConfigViewer);
  await screen.findByRole('button', { name: 'Files' });
}

describe('the Configuration layout on a wide window', () => {
  it('lays out the files, the file and its details, both toggles pressed, with the file named under the details heading', async () => {
    await showConfig(WIDE);

    expect(screen.getByRole('navigation', { name: 'Configuration files' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 2, name: 'Details' })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'Details' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Files' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: 'Details' })).toHaveAttribute('aria-pressed', 'true');
    expect(within(screen.getByRole('region', { name: 'Details' })).getByText('Overview')).toBeInTheDocument();
  });

  it('puts the toggles between the view switch and the menu', async () => {
    await showConfig(WIDE);

    const order = [screen.getByRole('radiogroup', { name: 'View' }), screen.getByRole('button', { name: 'Files' }), screen.getByRole('button', { name: 'Details' }), screen.getByRole('button', { name: 'More actions' })];

    for (const [before, after] of order.slice(0, -1).map((node, index) => [node, order[index + 1]])) {
      expect(before.compareDocumentPosition(after) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    }
  });

  it('hides the details from the toggle, remembers it, and shows them hidden after a remount', async () => {
    await showConfig(WIDE);

    await fireEvent.click(screen.getByRole('button', { name: 'Details' }));

    expect(screen.getByRole('button', { name: 'Details' })).toHaveAttribute('aria-pressed', 'false');
    expect(screen.queryByRole('region', { name: 'Details' })).toBeNull();
    expect(window.localStorage.getItem('harness.workspace.config.details')).toBe('false');

    document.body.replaceChildren();
    await showConfig(WIDE);

    expect(screen.getByRole('button', { name: 'Details' })).toHaveAttribute('aria-pressed', 'false');
    expect(screen.queryByRole('region', { name: 'Details' })).toBeNull();
  });

  it('hides the file list from its toggle and leaves the file in place', async () => {
    await showConfig(WIDE);
    const files = screen.getByRole('navigation', { name: 'Configuration files' });

    await fireEvent.click(screen.getByRole('button', { name: 'Files' }));

    expect(screen.getByRole('button', { name: 'Files' })).toHaveAttribute('aria-pressed', 'false');
    expect(files.closest('[inert]')).not.toBeNull();
    expect(window.localStorage.getItem('harness.workspace.config.list')).toBe('false');
    expect(screen.getByRole('article')).toBeInTheDocument();
  });

  it('is not held to a maximum width', async () => {
    await showConfig(WIDE);

    expect(screen.getByRole('article').closest('[class*="max-w-"]')).toBeNull();
  });
});

describe('the Configuration source view', () => {
  it('keeps the file title and its content off the borders, with the gutter the page view gives them', async () => {
    await showConfig(WIDE);
    const title = () => document.getElementById('section-title')!;
    expect(title().closest('.page-gutter')).not.toBeNull();

    await fireEvent.click(within(screen.getByRole('radiogroup', { name: 'View' })).getByRole('radio', { name: 'Source' }));

    expect(title().closest('.page-gutter')).not.toBeNull();
    expect(screen.getByRole('article')).toHaveClass('page-gutter');
  });

  it('keeps the file title above the scroller, in both views', async () => {
    await showConfig(WIDE);
    const scroller = () => screen.getByRole('article').parentElement!.parentElement!;

    expect(scroller().contains(document.getElementById('section-title'))).toBe(false);

    await fireEvent.click(within(screen.getByRole('radiogroup', { name: 'View' })).getByRole('radio', { name: 'Source' }));

    expect(scroller().contains(document.getElementById('section-title'))).toBe(false);
  });

  it('switches between the structured and the source view without rebuilding the file', async () => {
    await showConfig(WIDE);
    const article = screen.getByRole('article');

    await fireEvent.click(within(screen.getByRole('radiogroup', { name: 'View' })).getByRole('radio', { name: 'Source' }));
    expect(screen.getByRole('article')).toBe(article);

    await fireEvent.click(within(screen.getByRole('radiogroup', { name: 'View' })).getByRole('radio', { name: 'Structured' }));
    expect(screen.getByRole('article')).toBe(article);
  });
});

describe('the Configuration layout on a middle-width window', () => {
  it('keeps the details out until the toggle slides them over the file, and Escape puts them away with focus on the toggle', async () => {
    await showConfig(MIDDLE);
    const toggle = screen.getByRole('button', { name: 'Details' });
    expect(screen.queryByRole('region', { name: 'Details' })).toBeNull();
    expect(toggle).toHaveAttribute('aria-pressed', 'false');

    toggle.focus();
    await fireEvent.click(toggle);

    const details = await screen.findByRole('region', { name: 'Details' });
    expect(toggle).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('navigation', { name: 'Configuration files' })).toBeInTheDocument();

    within(details).getByRole('button', { name: 'Close' }).focus();
    await fireEvent.keyDown(details, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('region', { name: 'Details' })).toBeNull());
    expect(document.activeElement).toBe(toggle);
    expect(window.localStorage.length).toBe(0);
  });
});

describe('the Configuration layout on a narrow window', () => {
  it('keeps the files in a drawer until the toggle opens it, and Escape puts it away with focus on the toggle', async () => {
    await showConfig(NARROW);
    const toggle = screen.getByRole('button', { name: 'Files' });
    expect(screen.queryByRole('navigation', { name: 'Configuration files' })).toBeNull();
    expect(toggle).toHaveAttribute('aria-pressed', 'false');

    toggle.focus();
    await fireEvent.click(toggle);

    const drawer = await screen.findByRole('dialog', { name: 'Files' });
    expect(within(drawer).getByRole('navigation', { name: 'Configuration files' })).toBeInTheDocument();
    expect(toggle).toHaveAttribute('aria-pressed', 'true');

    await fireEvent.keyDown(drawer, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Files' })).toBeNull());
    expect(document.activeElement).toBe(toggle);
    expect(window.localStorage.length).toBe(0);
  });

  it('closes the drawer after a file is picked', async () => {
    await showConfig(NARROW);
    await fireEvent.click(screen.getByRole('button', { name: 'Files' }));
    const drawer = await screen.findByRole('dialog', { name: 'Files' });

    await fireEvent.click(within(drawer).getAllByRole('button').find((button) => button.hasAttribute('aria-current'))!);

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Files' })).toBeNull());
  });
});
