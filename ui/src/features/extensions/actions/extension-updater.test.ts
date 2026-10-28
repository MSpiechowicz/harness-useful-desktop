import { afterEach, describe, expect, it, vi } from 'vitest';

import type { CliResult } from '$lib/ipc/bindings/CliResult';
import type { ExtensionAction } from '$lib/ipc/bindings/ExtensionAction';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
import type { ToastInput } from '$lib/stores/toasts.svelte';

import { fakeTransport } from '../../../../tests/helpers';
import { CATALOG_REFRESH_MS, EXTENSION_UPDATE_FIRST_DELAY_MS, EXTENSION_UPDATE_INTERVAL_MS, PACKAGE_UPDATE_INTERVAL_MS } from '$lib/stores/live';

import { ipc } from '$lib/ipc/client';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { ExtensionUpdater, extensionUpdater } from './extension-updater';

const OMP = {
  info: { id: 'omp' },
  capabilities: { plugins: true, marketplaces: true, packages: false, cliActions: true },
} as HarnessSummary;
const CODEX = {
  info: { id: 'codex' },
  capabilities: { plugins: true, marketplaces: true, packages: false, cliActions: true },
} as HarnessSummary;
const PI = {
  info: { id: 'pi' },
  capabilities: { plugins: false, marketplaces: false, packages: true, cliActions: true },
} as HarnessSummary;
/** Installs and removes plugins, but never reports one outdated. */
const OPENCODE = {
  info: { id: 'opencode' },
  capabilities: { plugins: true, marketplaces: false, packages: false, cliActions: true },
} as HarnessSummary;
/** Lists plugins but runs nothing through its CLI. */
const READ_ONLY = {
  info: { id: 'opencode' },
  capabilities: { plugins: true, marketplaces: false, packages: false, cliActions: false },
} as HarnessSummary;

function outdated(...ids: string[]): ExtensionsView {
  return {
    marketplaces: [{ name: 'm', refreshable: true }],
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
    refreshCatalogs?: () => boolean;
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
    refreshCatalogs: options.refreshCatalogs ?? (() => true),
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

  it('leaves a harness without CLI actions alone', async () => {
    const { updater, log } = setup({ harnesses: [READ_ONLY], view: () => outdated('a@m') });

    await updater.check();

    expect(log).toEqual([]);
  });

  it('never upgrades OpenCode plugins, which it can only install and remove', async () => {
    const { updater, log } = setup({ harnesses: [OPENCODE], view: () => outdated('a@m') });

    await updater.check();

    expect(log).toEqual([]);
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

  it('does nothing while the user runs an action of their own', async () => {
    const { updater, log } = setup({ busy: () => true, view: () => outdated('a@m') });

    await updater.check();

    expect(log).toEqual([]);
  });

  it('refreshes the catalogs on their schedule and installs nothing while only the catalog setting is on', async () => {
    let clock = 0;
    const { updater, log, toasts } = setup({ enabled: () => false, view: () => outdated('a@m'), now: () => clock });

    await updater.check();
    expect(log).toEqual(['marketplaceUpdate']);

    clock += CATALOG_REFRESH_MS - 1;
    await updater.check();
    expect(log).toEqual(['marketplaceUpdate']);

    clock += 1;
    await updater.check();
    expect(log).toEqual(['marketplaceUpdate', 'marketplaceUpdate']);
    expect(toasts).toEqual([]);
  });

  it('refreshes the catalogs once before installing, and not on its own schedule, while only installs are on', async () => {
    const { updater, log } = setup({ refreshCatalogs: () => false, view: () => outdated('a@m') });

    await updater.check();

    expect(log).toEqual(['marketplaceUpdate', 'toast:Installing extension updates', 'pluginUpgrade', 'toast:Extensions updated']);
  });

  it.each([
    { name: 'both are on', enabled: true, refreshCatalogs: true },
    { name: 'only installing updates is on', enabled: true, refreshCatalogs: false },
    { name: 'only refreshing catalogs is on', enabled: false, refreshCatalogs: true },
  ])('reads the catalogs at most once every half hour however often it looks, while $name', async ({ enabled, refreshCatalogs }) => {
    let clock = 0;
    const { updater, log } = setup({ enabled: () => enabled, refreshCatalogs: () => refreshCatalogs, view: () => outdated(), now: () => clock });
    const refreshes = () => log.filter((type) => type === 'marketplaceUpdate').length;

    for (; clock < CATALOG_REFRESH_MS; clock += EXTENSION_UPDATE_INTERVAL_MS) {
      await updater.check();
    }
    expect(refreshes()).toBe(1);

    await updater.check();
    expect(refreshes()).toBe(2);
  });

  it('stops asking catalogs to refresh once both settings are switched off in the middle of a pass', async () => {
    let on = true;
    const { updater, log } = setup({
      view: () => ({ marketplaces: [{ name: 'a', refreshable: true }, { name: 'b', refreshable: true }, { name: 'c', refreshable: true }], plugins: [] }) as unknown as ExtensionsView,
      run: () => {
        on = false;

        return cli();
      },
      enabled: () => on,
      refreshCatalogs: () => on,
    });

    await updater.check();

    expect(log).toEqual(['marketplaceUpdate']);
  });

  it('refreshes only the catalogs the harness can refresh, leaving out built-in ones that would only fail', async () => {
    const refreshed: string[] = [];
    const { updater } = setup({
      harnesses: [CODEX],
      view: () =>
        ({
          marketplaces: [
            { name: 'openai-bundled', sourceType: 'local', refreshable: false },
            { name: 'harness-useful-skills', sourceType: 'git', refreshable: true },
          ],
          plugins: [],
        }) as unknown as ExtensionsView,
      run: (action) => {
        if (action.type === 'marketplaceUpdate') {
          refreshed.push(action.name ?? '');
        }

        return cli();
      },
    });

    await updater.check();

    expect(refreshed).toEqual(['harness-useful-skills']);
  });

  it('makes no request at all, even for PI packages, while both are off', async () => {
    let clock = 0;
    const { updater, log } = setup({ harnesses: [OMP, PI], enabled: () => false, refreshCatalogs: () => false, view: () => outdated('a@m'), now: () => clock });

    await updater.check();
    clock += CATALOG_REFRESH_MS * 2;
    await updater.check();

    expect(log).toEqual([]);
  });

  describe('the schedule', () => {
    afterEach(() => vi.useRealTimers());

    it('refreshes the catalogs soon after start and then every half hour, with installs off', async () => {
      vi.useFakeTimers();
      const { updater, log } = setup({ enabled: () => false });
      const stop = updater.start();

      await vi.advanceTimersByTimeAsync(EXTENSION_UPDATE_FIRST_DELAY_MS);
      expect(log).toEqual(['marketplaceUpdate']);

      // The looks in between find the catalogs fresh.
      await vi.advanceTimersByTimeAsync(CATALOG_REFRESH_MS - EXTENSION_UPDATE_INTERVAL_MS);
      expect(log).toEqual(['marketplaceUpdate']);

      await vi.advanceTimersByTimeAsync(EXTENSION_UPDATE_INTERVAL_MS);
      expect(log).toEqual(['marketplaceUpdate', 'marketplaceUpdate']);

      stop();
      await vi.advanceTimersByTimeAsync(CATALOG_REFRESH_MS);
      expect(log).toHaveLength(2);
    });

    it('requests nothing over time while both settings are off', async () => {
      vi.useFakeTimers();
      const { updater, log } = setup({ enabled: () => false, refreshCatalogs: () => false });
      const stop = updater.start();

      await vi.advanceTimersByTimeAsync(CATALOG_REFRESH_MS * 2);

      expect(log).toEqual([]);
      stop();
    });
  });

  it('leaves the packages of a PI machine alone while installs are off', async () => {
    const { updater, log } = setup({ harnesses: [PI], enabled: () => false });

    await updater.check();

    expect(log).toEqual([]);
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

describe('before the preferences are known', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    prefs.value = null;
    harness.summaries = [];
  });

  it('asks nothing of any harness, for neither switch is known to be on', async () => {
    prefs.value = null;
    harness.summaries = [{ ...OMP, detection: { installed: true } } as HarnessSummary];
    const invoke = vi.spyOn(ipc, 'invoke');

    await extensionUpdater.check();

    expect(invoke).not.toHaveBeenCalled();
  });
});
