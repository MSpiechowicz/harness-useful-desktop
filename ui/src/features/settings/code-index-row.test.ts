import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { CodeIndexState } from '$lib/ipc/bindings/CodeIndexState';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { IpcFailure } from '$lib/ipc/transport';
import { harness } from '$lib/stores/harness.svelte';
import { memoryCode } from '$lib/stores/memoryCode.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import SettingsView from './SettingsView.svelte';

interface Options {
  get?: () => CodeIndexState;
  set?: (on: boolean) => CodeIndexState;
}

/** A Harness Memory that answers the code index commands as the test says, and the rest as the mock does. */
function fakeCode({ get = () => ({ enabled: true, source: 'default' }), set = (on) => ({ enabled: on, source: 'file' }) }: Options = {}) {
  const passthrough = ipc.invoke.bind(ipc);

  return vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    switch (name) {
      case 'memory_code_index_get':
        return get() as CommandResult<K>;
      case 'memory_code_index_set':
        return set((args as CommandArgs<'memory_code_index_set'>).on) as CommandResult<K>;
      default:
        return passthrough(name, args);
    }
  });
}

const unsupported = (): never => {
  throw IpcFailure.from({ code: 'unsupported', message: 'The installed Harness Memory has no code index' });
};

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  Element.prototype.scrollIntoView = vi.fn();
  await harness.load();
});

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'off' });
  memoryRemote.value = null;
  memoryCode.reset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

const memorySection = (): HTMLElement => screen.getByRole('region', { name: 'Memory' });

describe('Code index in the Memory section', () => {
  it('is a switch with its description, right after the search rows, once the server answered', async () => {
    fakeCode();
    render(SettingsView);

    const code = await within(memorySection()).findByRole('switch', { name: 'Code index' });
    const switches = within(memorySection()).getAllByRole('switch');

    expect(code).toHaveAttribute('aria-checked', 'true');
    expect(within(memorySection()).getByText('Lets agents look up where code is defined and who uses it, from a local index of the project.')).toBeInTheDocument();
    expect(switches.indexOf(code)).toBeGreaterThan(switches.indexOf(within(memorySection()).getByRole('switch', { name: 'Project memory' })));
    expect(switches.indexOf(code)).toBeLessThan(switches.indexOf(within(memorySection()).getByRole('switch', { name: 'Remote memory' })));
  });

  it('is not there for a Harness Memory without a code index', async () => {
    const invoke = fakeCode({ get: unsupported });
    render(SettingsView);

    await within(memorySection()).findByRole('switch', { name: 'Remote memory' });
    await waitFor(() => expect(invoke.mock.calls.some(([name]) => name === 'memory_code_index_get')).toBe(true));

    expect(within(memorySection()).queryByRole('switch', { name: 'Code index' })).not.toBeInTheDocument();
  });

  it('switches off and on, and shows what the server reports', async () => {
    const invoke = fakeCode();
    render(SettingsView);

    const code = await within(memorySection()).findByRole('switch', { name: 'Code index' });

    await fireEvent.click(code);
    await waitFor(() => expect(code).toHaveAttribute('aria-checked', 'false'));
    await waitFor(() => expect(code).toBeEnabled());

    await fireEvent.click(code);
    await waitFor(() => expect(code).toHaveAttribute('aria-checked', 'true'));

    const sent = invoke.mock.calls.filter(([name]) => name === 'memory_code_index_set').map(([, args]) => args);

    expect(sent).toEqual([{ on: false }, { on: true }]);
  });

  it('goes back and shows the reason in the row when the server refuses', async () => {
    fakeCode({
      set: () => {
        throw IpcFailure.from({ code: 'process', message: 'The settings file is read-only' });
      },
    });
    render(SettingsView);

    const code = await within(memorySection()).findByRole('switch', { name: 'Code index' });

    await fireEvent.click(code);

    expect(await within(memorySection()).findByRole('alert')).toHaveTextContent('The settings file is read-only');
    expect(code).toHaveAttribute('aria-checked', 'true');
  });

  it('is disabled with a note when an environment variable decides', async () => {
    fakeCode({ get: () => ({ enabled: false, source: 'env' }) });
    render(SettingsView);

    const code = await within(memorySection()).findByRole('switch', { name: 'Code index' });

    expect(code).toBeDisabled();
    expect(code).toHaveAttribute('aria-checked', 'false');
    expect(within(memorySection()).getByText(/An environment variable controls this setting\./)).toBeInTheDocument();
  });
});
