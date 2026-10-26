import { describe, expect, it, vi } from 'vitest';

import type { SkillsMigrationReport } from '$lib/ipc/bindings/SkillsMigrationReport';
import type { ToastInput } from '$lib/stores/toasts.svelte';

import { fakeTransport } from '../../../tests/helpers';
import { SkillsNamesMigration } from './skills-migration';

function setup(answer: () => SkillsMigrationReport | Promise<SkillsMigrationReport>) {
  const toasts: ToastInput[] = [];
  const asked = vi.fn(answer);
  const migrated = vi.fn(async () => {});
  const migration = new SkillsNamesMigration({
    transport: fakeTransport({ skills_migrate_names: async () => asked() }),
    migrated,
    notifications: { push: (toast) => (toasts.push(toast), 0) },
  });

  return { migration, toasts, asked, migrated };
}

describe('Harness Skills names migration', () => {
  it('asks once per launch', async () => {
    const { migration, asked } = setup(() => ({ migrated: [], failed: [] }));

    await migration.run();
    await migration.run();

    expect(asked).toHaveBeenCalledTimes(1);
  });

  it('says nothing when there was nothing to move', async () => {
    const { migration, toasts, migrated } = setup(() => ({ migrated: [], failed: [] }));

    await migration.run();

    expect(toasts).toEqual([]);
    expect(migrated).not.toHaveBeenCalled();
  });

  it('names the harnesses that were moved and refreshes the ecosystem reading', async () => {
    const { migration, toasts, migrated } = setup(() => ({ migrated: ['claude', 'codex'], failed: [] }));

    await migration.run();

    expect(toasts).toEqual([{ title: 'Harness Skills renamed in Claude Code, Codex', tone: 'success' }]);
    expect(migrated).toHaveBeenCalledTimes(1);
  });

  it('names a failure with its reason, beside what did move', async () => {
    const { migration, toasts } = setup(() => ({ migrated: ['omp'], failed: [{ harness: 'codex', reason: 'the codex command was not found' }] }));

    await migration.run();

    expect(toasts).toEqual([{ title: 'Harness Skills renamed in OMP', description: 'Codex: the codex command was not found', tone: 'warning' }]);
  });

  it('reports a failure alone, with a fallback for a missing reason', async () => {
    const { migration, toasts, migrated } = setup(() => ({ migrated: [], failed: [{ harness: 'opencode', reason: '' }] }));

    await migration.run();

    expect(toasts).toEqual([{ title: 'Harness Skills could not be renamed in OpenCode', description: 'OpenCode: The installer did not say why.', tone: 'warning' }]);
    expect(migrated).not.toHaveBeenCalled();
  });

  it('shows nothing when the call itself fails', async () => {
    const { migration, toasts } = setup(() => {
      throw new Error('offline');
    });

    await migration.run();

    expect(toasts).toEqual([]);
  });
});
