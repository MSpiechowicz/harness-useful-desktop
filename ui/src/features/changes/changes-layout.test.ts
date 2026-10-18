import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { git } from '$lib/stores/git.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { restoreClientWidth, stubClientWidth } from '../../../tests/helpers';
import ChangesView from './ChangesView.svelte';

const WIDE = 1700;
const MIDDLE = 1400;
const NARROW = 1000;

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');

  await prefs.load();
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  await project.open(fixtures.recentProjects[0].path);
});

beforeEach(() => window.localStorage.clear());

afterEach(() => {
  restoreClientWidth();
  git.activate(null);
  nav.go('chat');
});

async function showChanges(width: number): Promise<void> {
  stubClientWidth(width);
  render(ChangesView);
  await screen.findByRole('button', { name: 'Files' });
}

describe('the Changes layout', () => {
  it.each([WIDE, MIDDLE, NARROW])('has a files toggle but never a details toggle at %i px', async (width) => {
    await showChanges(width);

    expect(screen.getByRole('button', { name: 'Files' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Details' })).toBeNull();
    expect(screen.queryByRole('region', { name: 'Details' })).toBeNull();
  });

  it('hides and shows the file list from its toggle on a wide window, and remembers it', async () => {
    await showChanges(WIDE);
    const toggle = screen.getByRole('button', { name: 'Files' });
    expect(toggle).toHaveAttribute('aria-pressed', 'true');

    await fireEvent.click(toggle);

    expect(toggle).toHaveAttribute('aria-pressed', 'false');
    expect(screen.getByTestId('file-list').closest('[inert]')).not.toBeNull();
    expect(window.localStorage.getItem('harness.workspace.changes.list')).toBe('false');

    await fireEvent.click(toggle);

    expect(toggle).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByTestId('file-list').closest('[inert]')).toBeNull();
    expect(window.localStorage.getItem('harness.workspace.changes.list')).toBe('true');
  });

  it('keeps the file list in a drawer on a narrow window until the toggle opens it', async () => {
    await showChanges(NARROW);
    const toggle = screen.getByRole('button', { name: 'Files' });
    expect(toggle).toHaveAttribute('aria-pressed', 'false');
    expect(screen.queryByTestId('file-list')).toBeNull();

    toggle.focus();
    await fireEvent.click(toggle);

    const drawer = await screen.findByRole('dialog', { name: 'Files' });
    expect(within(drawer).getByTestId('file-list')).toBeInTheDocument();
    expect(toggle).toHaveAttribute('aria-pressed', 'true');

    await fireEvent.keyDown(drawer, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Files' })).toBeNull());
    expect(document.activeElement).toBe(toggle);
    expect(window.localStorage.length).toBe(0);
  });
});
