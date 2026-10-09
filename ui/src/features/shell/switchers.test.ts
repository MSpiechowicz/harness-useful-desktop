import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { buildSwitchCommands } from './switch-commands';
import WorkspaceSwitcher from './WorkspaceSwitcher.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');

  await prefs.load();
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
});

describe('WorkspaceSwitcher: harness', () => {
  it('lists every harness and refuses the ones that are not installed', async () => {
    render(WorkspaceSwitcher);

    await fireEvent.click(screen.getByRole('button', { name: /Harness: Oh My Pi/ }));

    expect(screen.getByRole('menuitemradio', { name: /Oh My Pi/ })).toHaveAttribute('aria-checked', 'true');
    const missing = screen.getByRole('menuitemradio', { name: /^Pi.*Not installed/ });
    expect(missing).toHaveAttribute('aria-disabled', 'true');

    await fireEvent.click(missing);

    expect(prefs.value?.defaultHarness).not.toBe('pi');
    expect(screen.getByRole('menu', { name: 'Harness and project' })).toBeInTheDocument();
  });

  it('switches the daily driver once the other harness is installed, and closes on Escape', async () => {
    await harness.pickBinary('pi');
    render(WorkspaceSwitcher);

    await fireEvent.click(screen.getByRole('button', { name: /Switch harness or project/ }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: /^Pi/ }));

    await waitFor(() => expect(screen.getByRole('button', { name: /Harness: Pi/ })).toBeInTheDocument());
    expect(prefs.value?.defaultHarness).toBe('pi');
    expect(screen.queryByRole('menu')).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: /Switch harness or project/ }));
    await fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
    expect(screen.getByRole('button', { name: /Switch harness or project/ })).toHaveFocus();
  });
});

describe('WorkspaceSwitcher: project', () => {
  it('opens a folder, closes the project and forgets a recent one', async () => {
    render(WorkspaceSwitcher);
    const trigger = () => screen.getByRole('button', { name: /Switch harness or project/ });

    expect(trigger()).toHaveAccessibleName(/No project open/);

    await fireEvent.click(trigger());
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Open folder…' }));
    await waitFor(() => expect(trigger()).toHaveAccessibleName(/Project: harness-useful-desktop/));
    await waitFor(() => expect(trigger()).toHaveAttribute('aria-expanded', 'false'));

    await fireEvent.click(trigger());
    expect(screen.getByRole('menu', { name: 'Harness and project' })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Close project' }));
    await waitFor(() => expect(trigger()).toHaveAccessibleName(/No project open/));
    await waitFor(() => expect(trigger()).toHaveAttribute('aria-expanded', 'false'));

    await fireEvent.click(trigger());
    for (const missing of fixtures.recentProjects.filter((recent) => !recent.exists)) {
      expect(screen.getByRole('menuitemradio', { name: new RegExp(`^${missing.name}`) })).toHaveAttribute('aria-disabled', 'true');
    }

    const forgotten = fixtures.recentProjects[0].name;
    await fireEvent.click(screen.getByRole('button', { name: `Forget ${forgotten}` }));

    await waitFor(() => expect(screen.queryByRole('button', { name: `Forget ${forgotten}` })).toBeNull());
  });
});

describe('palette commands', () => {
  it('offers switching harness, opening a project and reopening a recent one', async () => {
    await project.refreshRecents();

    const labels = buildSwitchCommands().map((command) => command.label);

    expect(labels).toContain('Switch harness: OMP');
    expect(labels).toContain('Switch harness: PI');
    expect(labels).toContain('Open project…');
    expect(project.recents.length).toBeGreaterThan(0);
    for (const recent of project.recents) {
      expect(labels).toContain(`Open recent: ${recent.name}`);
    }
  });

  it('disables switching to a harness that is not installed', async () => {
    await harness.clearBinary('pi');

    const pi = buildSwitchCommands().find((command) => command.label === 'Switch harness: PI');

    expect(pi?.disabled).toBe(true);
  });
});
