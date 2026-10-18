import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, expect, vi } from 'vitest';

import type { RecommendedSetup } from '$lib/ipc/bindings/RecommendedSetup';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { hooksHandlers } from '$lib/ipc/mock/hooks-handlers';
import { type HookScenario, MockHooks } from '$lib/ipc/mock/hooks-state';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { startMockApp } from '../../../tests/mock-app';
import { hooks } from './hooks-data.svelte';
import HooksView from './HooksView.svelte';

export const PROJECT = fixtures.recentProjects[0].path;

/**
 * A hooks folder kept in memory that answers the hook commands like the real one (the mock backend's own), with the rest of
 * the app talking to the mock transport as usual. `memoryOn` is read at every list, so a test can switch it.
 */
export function fakeHooks(scenario: HookScenario = 'ok', memory = { on: true }, desktopMcpRegistered = false) {
  const state = new MockHooks(() => scenario, () => memory.on);
  const handlers = hooksHandlers(state);
  const passthrough = ipc.invoke.bind(ipc);

  const invoke = vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    const handler = (handlers as Record<string, (args: unknown) => unknown>)[name];

    if (handler) {
      return (await handler(args)) as CommandResult<K>;
    }

    const result = await passthrough(name, args);

    // Every harness found has the app's own entry in its MCP file.
    if (name === 'setup_status' && desktopMcpRegistered) {
      const setup = result as RecommendedSetup;
      const desktopMcp = { type: 'configured', file: '~/.omp/agent/mcp.json', name: 'harness-desktop' } as const;

      return { ...setup, harnesses: setup.harnesses.map((entry) => ({ ...entry, desktopMcp })) } as CommandResult<K>;
    }

    // Installing a member brings back its missing family hooks, as the backend does after the installer.
    if ((name === 'setup_install_dashboard' || name === 'setup_install_memory') && (result as { ok: boolean }).ok) {
      state.memberInstalled(name === 'setup_install_dashboard' ? 'harness-dashboard' : 'harness-memory');
    }

    return result as CommandResult<K>;
  });

  return { state, invoke, memory };
}

export function calls(invoke: ReturnType<typeof fakeHooks>['invoke'], name: string): unknown[] {
  return invoke.mock.calls.filter(([command]) => command === name).map(([, args]) => args);
}

/** The table row of a hook, by its name. */
export function rowOf(name: string): HTMLElement {
  const cell = screen.getByText(name, { selector: '[data-row] td div' });

  return cell.closest<HTMLElement>('[data-row]') as HTMLElement;
}

/** The row of a team hook, by its name: a team hook can share its name with a local one, so the row is found by its key. */
export function teamRowOf(name: string): HTMLElement {
  const row = [...document.querySelectorAll<HTMLElement>('[data-row^="team:"]')].find((candidate) => candidate.textContent?.includes(name));

  if (!row) {
    throw new Error(`no team hook is listed as ${name}`);
  }

  return row;
}

/** The row of a local hook, by its name, even when a team hook has the same one. */
export function localRowOf(name: string): HTMLElement {
  const row = [...document.querySelectorAll<HTMLElement>('[data-row]:not([data-row^="team:"])')].find((candidate) => candidate.textContent?.includes(name));

  if (!row) {
    throw new Error(`no local hook is listed as ${name}`);
  }

  return row;
}

/** Registers what every test of the Hooks view needs: the mock backend with the project open, and a clean view before and after each test. */
export function registerHooksHooks(): void {
  beforeAll(() => startMockApp(PROJECT));

  beforeEach(() => {
    viewState.reset('hooks');
    hooks.reset();
  });

  afterEach(() => {
    nav.go('chat');
    toasts.items = [];
    vi.restoreAllMocks();
  });
}

/** Shows the Hooks view once it has read the hooks (or failed to). */
export async function showHooks(): Promise<void> {
  nav.go('hooks');
  render(HooksView);
  await waitFor(() => expect(screen.queryByRole('status', { name: 'Loading hooks' })).not.toBeInTheDocument());
}

/** Opens a filter menu by its button and picks one of its choices. */
export async function pick(button: string, choice: RegExp): Promise<void> {
  await fireEvent.click(screen.getByRole('button', { name: button }));
  await fireEvent.click(await screen.findByRole('menuitemradio', { name: choice }));
}
