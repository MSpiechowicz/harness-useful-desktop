import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { DashboardInstallResult } from '$lib/ipc/bindings/DashboardInstallResult';
import type { InstallLogLine } from '$lib/ipc/bindings/InstallLogLine';
import type { MemoryScriptsAvailability } from '$lib/ipc/bindings/MemoryScriptsAvailability';
import { IpcFailure } from '$lib/ipc/transport';
import { nav } from '$lib/stores/nav.svelte';

import { calls, fakeMemory, type MemoryRelease, NO_NEWER_RELEASE, registerMemoryHooks } from './memory-test-support';
import ScriptsView from './ScriptsView.svelte';

registerMemoryHooks();

const LINE: InstallLogLine = { stream: 'stdout', line: '==> Downloading harness-memory (latest)' };

const result = (ok: boolean): DashboardInstallResult => ({
  ok,
  summary: ok ? 'Installed harness-memory 1.2.0.' : 'error: download failed',
  exitCode: ok ? 0 : 22,
  timedOut: false,
  truncated: false,
  version: ok ? '1.2.0' : null,
  command: 'install.sh',
});

interface Options {
  /** The installer waits for `release` before it ends. */
  hold?: boolean;
  /** What the probe finds once an install succeeded. */
  after?: MemoryScriptsAvailability;
  /** The latest Harness Memory release next to the installed one. A newer one by default. */
  memoryRelease?: MemoryRelease;
}

/**
 * A Harness Memory that is `before` until an install succeeds. Each install streams one line, then ends the way the next
 * entry of `outcomes` says: a result, or a failure it throws.
 */
function installable(before: MemoryScriptsAvailability, outcomes: (DashboardInstallResult | IpcFailure)[], { hold = false, after = { kind: 'available' }, memoryRelease }: Options = {}) {
  let installed = false;
  let release = () => {};
  const gate = hold ? new Promise<void>((resolve) => (release = resolve)) : Promise.resolve();

  const invoke = fakeMemory(() => (installed ? after : before), undefined, memoryRelease);
  const rest = invoke.getMockImplementation()!;

  invoke.mockImplementation(async (name, args) => {
    if (name !== 'setup_install_memory') {
      return rest(name, args);
    }

    (args as { onLog: (line: InstallLogLine) => void }).onLog(LINE);
    await gate;

    const outcome = outcomes.shift()!;
    if (outcome instanceof IpcFailure) {
      throw outcome;
    }

    installed ||= outcome.ok;

    return outcome as never;
  });

  return { invoke, release: () => release() };
}

const installs = (invoke: ReturnType<typeof fakeMemory>) => calls(invoke, 'setup_install_memory') as { confirmed: boolean }[];

function open(): void {
  nav.go('scripts');
  render(ScriptsView);
}

/** Clicks the page's button, then the same action in the question that follows. */
async function confirm(name: string): Promise<void> {
  await fireEvent.click(await screen.findByRole('button', { name }));

  const dialog = await screen.findByRole('dialog');
  await fireEvent.click(within(dialog).getByRole('button', { name }));
}

describe('updating Harness Memory from the Notes and Scripts views', () => {
  it('asks first, then runs the installer with the user confirmation, shows its output and lists the scripts', async () => {
    const { invoke, release } = installable({ kind: 'outdated' }, [result(true)], { hold: true });
    open();

    await fireEvent.click(await screen.findByRole('button', { name: 'Update Harness Memory' }));

    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText(/curl .*install\.sh/)).toBeInTheDocument();
    expect(within(dialog).getByText(/replacing an older one/)).toBeInTheDocument();
    expect(installs(invoke)).toEqual([]);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Update Harness Memory' }));

    expect(await screen.findByText('==> Downloading harness-memory (latest)')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Updating Harness Memory…');
    expect(installs(invoke)).toEqual([expect.objectContaining({ confirmed: true })]);

    const reads = calls(invoke, 'memory_scripts_list').length;
    release();

    expect(await screen.findByRole('grid', { name: 'Scripts' })).toBeInTheDocument();
    expect(calls(invoke, 'memory_scripts_list').length).toBeGreaterThan(reads);
  });

  it('installs a missing Harness Memory the same way', async () => {
    const { invoke } = installable({ kind: 'notInstalled' }, [result(true)]);
    open();

    await confirm('Install Harness Memory');

    expect(await screen.findByRole('grid', { name: 'Scripts' })).toBeInTheDocument();
    expect(installs(invoke)).toEqual([expect.objectContaining({ confirmed: true })]);
  });

  it('runs nothing when the question is cancelled', async () => {
    const { invoke } = installable({ kind: 'outdated' }, []);
    open();

    await fireEvent.click(await screen.findByRole('button', { name: 'Update Harness Memory' }));
    await fireEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Cancel' }));

    expect(installs(invoke)).toEqual([]);
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(screen.getByRole('button', { name: 'Update Harness Memory' })).toBeInTheDocument();
  });

  it('shows why the installer failed and asks again before it runs it a second time', async () => {
    const { invoke } = installable({ kind: 'outdated' }, [result(false), result(true)]);
    open();

    await confirm('Update Harness Memory');

    expect(await screen.findByRole('alert')).toHaveTextContent('The installer did not finish: error: download failed');
    expect(screen.getByRole('log')).toHaveTextContent('==> Downloading harness-memory (latest)');

    await fireEvent.click(screen.getByRole('button', { name: 'Try again' }));

    const dialog = await screen.findByRole('dialog');
    expect(installs(invoke)).toHaveLength(1);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Update Harness Memory' }));

    expect(await screen.findByRole('grid', { name: 'Scripts' })).toBeInTheDocument();
    expect(installs(invoke)).toEqual([expect.objectContaining({ confirmed: true }), expect.objectContaining({ confirmed: true })]);
  });

  it('shows a refused install and still leads to the Ecosystem page', async () => {
    installable({ kind: 'outdated' }, [new IpcFailure({ code: 'invalidInput', message: 'harness-memory is already being installed' })]);
    open();

    await confirm('Update Harness Memory');

    expect(await screen.findByRole('alert')).toHaveTextContent('The installer did not finish: harness-memory is already being installed');
    await fireEvent.click(screen.getByRole('button', { name: 'Open Ecosystem' }));

    expect(nav.current).toBe('ecosystem');
  });

  it('says so when the installer finished but Harness Memory is still unusable here', async () => {
    installable({ kind: 'outdated' }, [result(true)], { after: { kind: 'outdated' } });
    open();

    await confirm('Update Harness Memory');

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('still cannot use Harness Memory'));
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });
});

describe('Harness Memory without a newer release', () => {
  it('still offers the update and says there is nothing newer when it is pressed', async () => {
    const { invoke } = installable({ kind: 'outdated' }, [], { memoryRelease: NO_NEWER_RELEASE });
    open();

    expect(await screen.findByText('Your memory needs an update')).toBeInTheDocument();
    await fireEvent.click(await screen.findByRole('button', { name: 'Update Harness Memory' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('There is no newer version to install yet. Try again later.');
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(installs(invoke)).toEqual([]);

    await fireEvent.click(screen.getByRole('button', { name: 'Open Ecosystem' }));

    expect(nav.current).toBe('ecosystem');
  });

  it('runs nothing while the releases could not be looked up', async () => {
    const { invoke } = installable({ kind: 'outdated' }, []);
    const rest = invoke.getMockImplementation()!;
    invoke.mockImplementation(async (name, args) => {
      if (name === 'ecosystem_releases') {
        throw new IpcFailure({ code: 'internal', message: 'offline' });
      }

      return rest(name, args);
    });
    open();

    await fireEvent.click(await screen.findByRole('button', { name: 'Update Harness Memory' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('There is no newer version to install yet.');
    expect(installs(invoke)).toEqual([]);
  });

  it('still installs a missing Harness Memory whatever the releases say', async () => {
    const { invoke } = installable({ kind: 'notInstalled' }, [result(true)], { memoryRelease: NO_NEWER_RELEASE });
    open();

    await confirm('Install Harness Memory');

    expect(await screen.findByRole('grid', { name: 'Scripts' })).toBeInTheDocument();
    expect(installs(invoke)).toHaveLength(1);
  });

  it('checks again after the question and runs nothing when the release is no longer newer', async () => {
    let newer = true;
    const { invoke } = installable({ kind: 'outdated' }, [result(true)]);
    const rest = invoke.getMockImplementation()!;
    invoke.mockImplementation(async (name, args) => (name === 'ecosystem_releases' ? ((await rest(name, args)) as { id: string }[]).map((entry) => (entry.id === 'memory' ? { ...entry, updateAvailable: newer } : entry)) : rest(name, args)) as never);
    open();

    await fireEvent.click(await screen.findByRole('button', { name: 'Update Harness Memory' }));
    const dialog = await screen.findByRole('dialog');

    // The release lookup changes while the question is open, e.g. the source build was installed meanwhile.
    newer = false;
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Update Harness Memory' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('There is no newer version to install yet.');
    expect(installs(invoke)).toEqual([]);
  });
});
