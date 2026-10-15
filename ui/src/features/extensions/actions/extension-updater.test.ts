import { describe, expect, it, vi } from 'vitest';

import type { CliResult } from '$lib/ipc/bindings/CliResult';
import type { ExtensionAction } from '$lib/ipc/bindings/ExtensionAction';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
import type { ToastInput } from '$lib/stores/toasts.svelte';

import { fakeTransport } from '../../../../tests/helpers';
import { ExtensionUpdater, PACKAGE_UPDATE_INTERVAL_MS } from './extension-updater';

const OMP = {
  info: { id: 'omp' },
  capabilities: { plugins: true, marketplaces: true, packages: false },
} as HarnessSummary;
const PI = {
  info: { id: 'pi' },
  capabilities: { plugins: false, marketplaces: false, packages: true },
} as HarnessSummary;

function outdated(...ids: string[]): ExtensionsView {
  return {
    marketplaces: [{ name: 'm' }],
    plugins: ids.map((id) => ({
      id,
      name: id.split('@')[0],
      updateAvailable: true,
      latestVersion: '2.0.0',
    })),
  } as ExtensionsView;
}

function cli(fields: Partial<CliResult> = {}): CliResult {
  return {
    ok: true,
    summary: null,
    exitCode: 0,
    stdout: '',
    stderr: '',
    truncated: false,
    timedOut: false,
    argv: [],
    command: [],
    ...fields,
  };
}

/** An updater over a scripted harness CLI; `log` records every action and toast in order. */
function setup(
  options: {
    harnesses?: HarnessSummary[];
    view?: () => ExtensionsView;
    run?: (action: ExtensionAction) => CliResult;
    enabled?: () => boolean;
    busy?: () => boolean;
    now?: () => number;
  } = {},
) {
  const log: string[] = [];
  const toasts: ToastInput[] = [];
  const upgraded = vi.fn(async () => {});
  const transport = fakeTransport({
    extension_action: ({ action }: { action: ExtensionAction }) => {
      log.push(action.type);

      return (options.run ?? (() => cli()))(action);
    },
    extensions_view: () => (options.view ?? (() => outdated()))(),
  });
  const updater = new ExtensionUpdater({
    transport,
    installed: () => options.harnesses ?? [OMP],
    enabled: options.enabled ?? (() => true),
    busy: options.busy ?? (() => false),
    upgraded,
    notifications: {
      push: (toast) => {
        log.push(`toast:${toast.title}`);
        toasts.push(toast);

        return 0;
      },
    },
    now: options.now,
  });

  return { updater, log, toasts, upgraded };
}

describe('Extension auto-update', () => {
  it('refreshes the catalogs, announces the outdated plugins, then upgrades them', async () => {
    const { updater, log, toasts, upgraded } = setup({
      view: () => outdated('a@m', 'b@m'),
    });

    await updater.check();

    expect(log).toEqual(['marketplaceUpdate', 'toast:Installing extension updates', 'pluginUpgrade', 'toast:Extensions updated']);
    expect(toasts[0].description).toBe('a, b');
    expect(upgraded).toHaveBeenCalledWith('omp');
  });

  it('does not repeat a batch that went through but still shows as outdated', async () => {
    const { updater, log } = setup({ view: () => outdated('a@m') });

    await updater.check();
    await updater.check();

    expect(log.filter((entry) => entry === 'pluginUpgrade')).toHaveLength(1);
  });

  it('stays silent when nothing is outdated', async () => {
    const { updater, log } = setup();

    await updater.check();

    expect(log).toEqual(['marketplaceUpdate']);
  });

  it('reports a failure OMP prints with exit code 0, and does not retry the same batch', async () => {
    const run = (action: ExtensionAction) => (action.type === 'pluginUpgrade' ? cli({ stdout: '✘ Failed to upgrade a: boom\n' }) : cli());
    const { updater, log, toasts } = setup({
      view: () => outdated('a@m'),
      run,
    });

    await updater.check();
    await updater.check();

    expect(toasts.map((toast) => toast.title)).toEqual(['Installing extension updates', 'An extension update failed']);
    expect(toasts[1].description).toBe('✘ Failed to upgrade a: boom');
    expect(log.filter((entry) => entry === 'pluginUpgrade')).toHaveLength(1);
  });

  it('tries again once a different set of plugins is outdated', async () => {
    let ids = ['a@m'];
    const run = (action: ExtensionAction) => (action.type === 'pluginUpgrade' ? cli({ ok: false, summary: 'no' }) : cli());
    const { updater, log } = setup({ view: () => outdated(...ids), run });

    await updater.check();
    ids = ['a@m', 'b@m'];
    await updater.check();

    expect(log.filter((entry) => entry === 'pluginUpgrade')).toHaveLength(2);
  });

  it('does nothing while switched off or while the user runs an action of their own', async () => {
    const off = setup({ enabled: () => false, view: () => outdated('a@m') });
    const busy = setup({ busy: () => true, view: () => outdated('a@m') });

    await off.updater.check();
    await busy.updater.check();

    expect(off.log).toEqual([]);
    expect(busy.log).toEqual([]);
  });

  it('updates PI packages at most once an hour and only announces a failure', async () => {
    let clock = 0;
    const { updater, log, toasts } = setup({
      harnesses: [PI],
      now: () => clock,
    });

    await updater.check();
    clock += PACKAGE_UPDATE_INTERVAL_MS - 1;
    await updater.check();
    clock += 1;
    await updater.check();

    expect(log).toEqual(['packageUpdate', 'packageUpdate']);
    expect(toasts).toEqual([]);

    const failing = setup({
      harnesses: [PI],
      run: () => cli({ ok: false, summary: 'npm unreachable' }),
    });
    await failing.updater.check();

    expect(failing.toasts).toMatchObject([
      {
        title: 'A package update failed',
        description: 'npm unreachable',
        tone: 'warning',
      },
    ]);
  });
});
