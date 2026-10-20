import { describe, expect, it, vi } from 'vitest';

import type { DashboardInstallResult } from '$lib/ipc/bindings/DashboardInstallResult';
import type { EcosystemId } from '$lib/ipc/bindings/EcosystemId';
import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';
import type { ToastInput } from '$lib/stores/toasts.svelte';

import { fakeTransport } from '../../../tests/helpers';
import { FamilyUpdater } from './family-updater';

const NAMES: Record<EcosystemId, string> = { dashboard: 'Harness Dashboard', memory: 'Harness Memory', skills: 'Harness Skills', sidebar: 'Harness Sidebar' };

/** A family member as the ecosystem reading reports it. */
function member(id: EcosystemId, fields: Partial<EcosystemItem> = {}): EcosystemItem {
  return { id, name: NAMES[id], installed: true, version: '1.0.0', latestVersion: '1.1.0', updateAvailable: true, ...fields } as EcosystemItem;
}

function installed(version: string | null = '1.1.0', fields: Partial<DashboardInstallResult> = {}): DashboardInstallResult {
  return { ok: true, summary: null, exitCode: 0, timedOut: false, truncated: false, version, command: 'install', ...fields };
}

/** An updater over a scripted ecosystem reading and installers; `log` records the installs and toasts in order. */
function setup(
  options: {
    items?: EcosystemItem[];
    /** The reading at each look; wins over `items` so a test can move it along. */
    reading?: () => EcosystemItem[];
    enabled?: () => boolean;
    busy?: () => boolean;
    install?: (id: 'dashboard' | 'memory') => DashboardInstallResult | Promise<DashboardInstallResult>;
  } = {},
) {
  const log: string[] = [];
  const toasts: ToastInput[] = [];
  const updated = vi.fn(async (id: EcosystemId) => {
    log.push(`updated:${id}`);
  });
  const run = (id: 'dashboard' | 'memory') => async () => {
    log.push(`start:${id}`);
    const result = await (options.install ?? (() => installed()))(id);
    log.push(`end:${id}`);

    return result;
  };
  const transport = fakeTransport({
    setup_install_dashboard: run('dashboard'),
    setup_install_memory: run('memory'),
    // Never reached: the Harness Skills for OpenCode update from OpenCode itself.
    setup_install_opencode_skills: async () => {
      log.push('start:skills');

      return installed();
    },
  });
  const updater = new FamilyUpdater({
    transport,
    status: () => ({ items: options.reading?.() ?? options.items ?? [member('dashboard'), member('memory')] }),
    enabled: options.enabled ?? (() => true),
    busy: options.busy ?? (() => false),
    updated,
    notifications: {
      push: (toast) => {
        log.push(`toast:${toast.title}`);
        toasts.push(toast);

        return 0;
      },
    },
  });

  return { updater, log, toasts, updated };
}

describe('Harness family auto-update', () => {
  it('leaves the Harness Skills alone, since OpenCode updates them itself', async () => {
    const { updater, log, toasts } = setup({ items: [member('skills')] });

    await updater.check();

    expect(log).toEqual([]);
    expect(toasts).toEqual([]);
  });

  it('updates an installed member that has a newer release, then refreshes what depends on it', async () => {
    const { updater, log, toasts, updated } = setup({ items: [member('memory')] });

    await updater.check();

    expect(log).toEqual(['start:memory', 'end:memory', 'toast:Harness Memory updated to 1.1.0', 'updated:memory']);
    expect(toasts[0].tone).toBe('success');
    expect(updated).toHaveBeenCalledWith('memory');
  });

  it('names the version the installer reports, and falls back to the release it was after', async () => {
    const reported = setup({ items: [member('dashboard')], install: () => installed('1.2.3') });
    const silent = setup({ items: [member('dashboard')], install: () => installed(null) });

    await reported.updater.check();
    await silent.updater.check();

    expect(reported.toasts[0].title).toBe('Harness Dashboard updated to 1.2.3');
    expect(silent.toasts[0].title).toBe('Harness Dashboard updated to 1.1.0');
  });

  it('never installs a member that is not installed', async () => {
    const { updater, log } = setup({ items: [member('memory', { installed: false, version: null }), member('dashboard', { installed: false })] });

    await updater.check();

    expect(log).toEqual([]);
  });

  it('leaves a member alone that has no newer release', async () => {
    const { updater, log } = setup({ items: [member('memory', { updateAvailable: false }), member('dashboard', { updateAvailable: false })] });

    await updater.check();

    expect(log).toEqual([]);
  });

  it('only ever installs the dashboard and the memory server, not the skills or sidebar', async () => {
    const { updater, log } = setup({ items: [member('skills'), member('sidebar')] });

    await updater.check();

    expect(log).toEqual([]);
  });

  it('does nothing while the setting is off or the first-run setup is installing', async () => {
    const off = setup({ enabled: () => false });
    const busy = setup({ busy: () => true });

    await off.updater.check();
    await busy.updater.check();

    expect(off.log).toEqual([]);
    expect(busy.log).toEqual([]);
  });

  it('updates the members one after the other, never at the same time', async () => {
    const { updater, log } = setup({
      install: async () => {
        await new Promise((resolve) => setTimeout(resolve, 5));

        return installed();
      },
    });

    await updater.check();

    expect(log.filter((entry) => /^(start|end):/.test(entry))).toEqual(['start:dashboard', 'end:dashboard', 'start:memory', 'end:memory']);
  });

  it('stops between members when the setting is switched off meanwhile', async () => {
    let enabled = true;
    const { updater, log } = setup({
      enabled: () => enabled,
      install: () => {
        enabled = false;

        return installed();
      },
    });

    await updater.check();

    expect(log.filter((entry) => entry.startsWith('start:'))).toEqual(['start:dashboard']);
  });

  it('reports a failed installer with a danger toast, leaves the reading alone and does not try that release again', async () => {
    const { updater, log, toasts, updated } = setup({
      items: [member('memory')],
      install: () => installed(null, { ok: false, summary: 'checksum mismatch', exitCode: 1 }),
    });

    await updater.check();
    await updater.check();

    expect(log.filter((entry) => entry.startsWith('start:'))).toEqual(['start:memory']);
    expect(toasts).toEqual([{ title: 'Harness Memory update failed', description: 'checksum mismatch', tone: 'danger' }]);
    expect(updated).not.toHaveBeenCalled();
  });

  it('reports an installer that could not run as a failure too, and one failure does not stop the next member', async () => {
    const { updater, log, toasts } = setup({
      install: (id) => {
        if (id === 'dashboard') {
          throw new Error('spawn failed');
        }

        return installed();
      },
    });

    await updater.check();

    expect(toasts.map((toast) => toast.title)).toEqual(['Harness Dashboard update failed', 'Harness Memory updated to 1.1.0']);
    expect(toasts[0].description).toBe('spawn failed');
    expect(log).toContain('start:memory');
  });

  it('tries a newer release of a member that failed before', async () => {
    let items = [member('memory')];
    let fail = true;
    const { updater, log } = setup({
      reading: () => items,
      install: () => (fail ? installed(null, { ok: false, summary: 'offline', exitCode: 1 }) : installed('1.2.0')),
    });

    await updater.check();
    fail = false;
    items = [member('memory', { latestVersion: '1.2.0' })];
    await updater.check();

    expect(log.filter((entry) => entry.startsWith('start:'))).toEqual(['start:memory', 'start:memory']);
    expect(log).toContain('toast:Harness Memory updated to 1.2.0');
  });

  it('skips quietly while the same member is already being installed, and tries again at the next reading', async () => {
    let running = true;
    const { updater, log, toasts } = setup({
      items: [member('memory')],
      install: () => {
        if (running) {
          throw { code: 'invalidInput', message: 'harness-memory is already being installed' };
        }

        return installed();
      },
    });

    await updater.check();

    expect(toasts).toEqual([]);

    running = false;
    await updater.check();

    expect(log.filter((entry) => entry.startsWith('start:'))).toEqual(['start:memory', 'start:memory']);
    expect(toasts.map((toast) => toast.title)).toEqual(['Harness Memory updated to 1.1.0']);
  });

  it('does not start a second run while one is going, and looks once more when it ends', async () => {
    const { updater, log } = setup({
      items: [member('memory')],
      install: async () => {
        await new Promise((resolve) => setTimeout(resolve, 5));

        return installed();
      },
    });

    await Promise.all([updater.check(), updater.check()]);

    expect(log.filter((entry) => entry.startsWith('start:'))).toEqual(['start:memory']);
  });
});
