import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { CodeIndexState } from '$lib/ipc/bindings/CodeIndexState';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { IpcFailure } from '$lib/ipc/transport';
import { codeGraph } from '$lib/stores/codeGraph.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import SettingsView from './SettingsView.svelte';

interface Options {
  get?: () => CodeIndexState;
  set?: (on: boolean) => CodeIndexState;
}

/** A Harness Code that answers the code index commands as the test says, and the rest as the mock does. */
function fakeCode({ get = () => ({ enabled: true, source: 'default' }), set = (on) => ({ enabled: on, source: 'file' }) }: Options = {}) {
  const passthrough = ipc.invoke.bind(ipc);

  return vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    switch (name) {
      case 'code_index_get':
        return get() as CommandResult<K>;
      case 'code_index_set':
        return set((args as CommandArgs<'code_index_set'>).on) as CommandResult<K>;
      default:
        return passthrough(name, args);
    }
  });
}

const failing = (code: 'unsupported' | 'notFound'): (() => never) => () => {
  throw IpcFailure.from({ code, message: 'Harness Code is missing' });
};

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  Element.prototype.scrollIntoView = vi.fn();
  await harness.load();
});

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'off' });
  prefs.dismissCodeManualRemovals();
  memoryRemote.value = null;
  codeGraph.reset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

const section = (): HTMLElement => screen.getByRole('region', { name: 'Code' });

describe('Code section', () => {
  it('is its own entry in the section nav, right after Knowledge, with the Code graph switch and its description', async () => {
    fakeCode();
    render(SettingsView);
    const nav = screen.getByRole('navigation', { name: 'Settings sections' });
    const entries = within(nav).getAllByRole('button').map((button) => button.textContent?.trim());

    expect(entries.indexOf('Code')).toBe(entries.indexOf('Knowledge') + 1);

    await fireEvent.click(within(nav).getByRole('button', { name: 'Code' }));

    const code = await within(section()).findByRole('switch', { name: 'Code graph' });

    expect(code).toHaveAttribute('aria-checked', 'true');
    expect(within(section()).getByText(/Lets your agents look up where code is defined and who uses it/)).toBeInTheDocument();
  });

  it('is no longer a row of the Memory section', async () => {
    fakeCode();
    render(SettingsView);

    await within(section()).findByRole('switch', { name: 'Code graph' });

    expect(within(screen.getByRole('region', { name: 'Memory' })).queryByRole('switch', { name: /Code/ })).not.toBeInTheDocument();
  });

  it('registers the server and then turns the index off and on, through the commands named for it', async () => {
    const invoke = fakeCode();
    render(SettingsView);

    const code = await within(section()).findByRole('switch', { name: 'Code graph' });

    await fireEvent.click(code);
    await waitFor(() => expect(code).toHaveAttribute('aria-checked', 'false'));
    await waitFor(() => expect(code).toBeEnabled());
    expect(prefs.value?.codeEnabled).toBe(false);

    await fireEvent.click(code);
    await waitFor(() => expect(code).toHaveAttribute('aria-checked', 'true'));
    await waitFor(() => expect(code).toBeEnabled());
    expect(prefs.value?.codeEnabled).toBe(true);

    const sent = invoke.mock.calls.filter(([name]) => name === 'code_set_enabled' || name === 'code_index_set').map(([name, args]) => [name, args]);

    expect(sent).toEqual([
      ['code_set_enabled', { enabled: false }],
      ['code_index_set', { on: false }],
      ['code_set_enabled', { enabled: true }],
      ['code_index_set', { on: true }],
    ]);
    expect(invoke).not.toHaveBeenCalledWith('memory_set_enabled', expect.anything());
  });

  it('is off while only one of the two is off', async () => {
    fakeCode({ get: () => ({ enabled: false, source: 'file' }) });
    render(SettingsView);

    expect(await within(section()).findByRole('switch', { name: 'Code graph' })).toHaveAttribute('aria-checked', 'false');
  });

  it('goes back and shows the reason in the row when the server refuses', async () => {
    fakeCode({
      set: () => {
        throw IpcFailure.from({ code: 'process', message: 'The settings file is read-only' });
      },
    });
    render(SettingsView);

    const code = await within(section()).findByRole('switch', { name: 'Code graph' });

    await fireEvent.click(code);

    expect(await within(section()).findByRole('alert')).toHaveTextContent('The settings file is read-only');
    await waitFor(() => expect(code).toHaveAttribute('aria-checked', 'false'));
  });

  it('is disabled with a note when an environment variable decides', async () => {
    fakeCode({ get: () => ({ enabled: false, source: 'env' }) });
    render(SettingsView);

    const code = await within(section()).findByRole('switch', { name: 'Code graph' });

    expect(code).toBeDisabled();
    expect(code).toHaveAttribute('aria-checked', 'false');
    expect(within(section()).getByText(/An environment variable controls this setting\./)).toBeInTheDocument();
  });

  it('installs a missing Harness Code right here after the user confirmed what it runs', async () => {
    render(SettingsView);

    expect(await within(section()).findByText('Harness Code is not installed')).toBeInTheDocument();
    // there is no index to claim on: the switch is off and waits for the install
    const code = within(section()).getByRole('switch', { name: 'Code graph' });
    expect(code).toHaveAttribute('aria-checked', 'false');
    expect(code).toBeDisabled();
    await fireEvent.click(within(section()).getByRole('button', { name: 'Install' }));

    const confirmation = await screen.findByRole('dialog', { name: 'Install Harness Code?' });
    expect(within(confirmation).getByText(/harness-useful-code/)).toBeInTheDocument();
    await fireEvent.click(within(confirmation).getByRole('button', { name: 'Install' }));

    await waitFor(() => expect(within(section()).queryByText('Harness Code is not installed')).toBeNull(), { timeout: 3000 });
    expect(codeGraph.availability).toBe('ready');
  });

  it('offers the update of a Harness Code without the code index, and says nothing when it cannot tell', async () => {
    fakeCode({ get: failing('unsupported') });
    render(SettingsView);

    expect(await within(section()).findByText('Harness Code is out of date')).toBeInTheDocument();
    expect(within(section()).getByRole('button', { name: 'Update' })).toBeInTheDocument();
    expect(within(section()).getByRole('button', { name: 'Open Ecosystem' })).toBeInTheDocument();
  });
});
