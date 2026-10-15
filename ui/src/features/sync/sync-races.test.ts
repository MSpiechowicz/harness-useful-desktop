import { describe, expect, it, vi } from 'vitest';

import type { SyncExported } from '$lib/ipc/bindings/SyncExported';
import type { SyncImportPlan } from '$lib/ipc/bindings/SyncImportPlan';
import type { CommandName } from '$lib/ipc/commands';
import type { Transport } from '$lib/ipc/transport';

import { ExportController } from './sync-export-state.svelte';
import { ImportController } from './sync-import-state.svelte';

/** A promise the test settles by hand. */
function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => (resolve = done));

  return { promise, resolve };
}

function transportOf(answers: Partial<Record<CommandName, (args: unknown) => unknown>>): Transport {
  return { invoke: vi.fn(async (name: CommandName, args: unknown) => answers[name]?.(args)) } as unknown as Transport;
}

const exported = (code: string): SyncExported => ({ code, itemCount: 1, bytes: code.length });

const preview = {
  sections: [
    {
      harness: 'omp',
      items: [
        { id: 'a', kind: 'marketplace', label: 'a', sub: null, runsProgram: false },
        { id: 'b', kind: 'marketplace', label: 'b', sub: null, runsProgram: false },
      ],
    },
  ],
  excluded: [],
};

describe('Export selection while a code is being built', () => {
  it('discards a code built for a selection that changed in the meantime', async () => {
    const pending = deferred<SyncExported>();
    const clipboard = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: clipboard }, configurable: true });

    const transport = transportOf({ sync_export_preview: () => preview, sync_export: () => pending.promise });
    const flow = new ExportController(transport);

    await flow.open();

    const copying = flow.copy();

    flow.toggle('b', false);
    pending.resolve(exported('hud1:old'));
    await copying;

    expect(flow.code).toBeNull();
    expect(flow.copied).toBe(false);
    expect(clipboard).not.toHaveBeenCalled();
    expect(flow.busy).toBe(false);
  });

  it('discards the file path of a save for a selection that changed in the meantime', async () => {
    const pending = deferred<string | null>();
    const transport = transportOf({ sync_export_preview: () => preview, sync_export_save: () => pending.promise });
    const flow = new ExportController(transport);

    await flow.open();

    const saving = flow.save();

    flow.toggle('a', false);
    pending.resolve('/tmp/old.json');
    await saving;

    expect(flow.savedTo).toBeNull();
    expect(flow.saveCancelled).toBe(false);
  });
});

describe('Import preview across sessions', () => {
  const plan = (planId: string): SyncImportPlan => ({ planId, app: null, createdAt: null, sections: [], unsupported: [], invalidCount: 0 });

  it('ignores a late answer after the dialog was closed and reopened', async () => {
    const pending = deferred<SyncImportPlan | null>();
    const transport = transportOf({ sync_import_preview: () => pending.promise });
    const flow = new ImportController(transport);

    flow.text = 'hud1:old';

    const reviewing = flow.review();

    flow.reset();
    pending.resolve(plan('stale'));
    await reviewing;

    expect(flow.stage).toBe('input');
    expect(flow.plan).toBeNull();
    expect(flow.text).toBe('');
    expect(flow.reviewing).toBe(false);
  });

  it('does not let a late failure or a late finish of the old session touch the new one', async () => {
    const first = deferred<SyncImportPlan | null>();
    const second = deferred<SyncImportPlan | null>();
    const answers = [first.promise, second.promise];
    const transport = transportOf({ sync_import_preview: () => answers.shift() });
    const flow = new ImportController(transport);

    flow.text = 'hud1:one';

    const old = flow.review();

    flow.reset();
    flow.text = 'hud1:two';

    const fresh = flow.review();

    first.resolve(null);
    await old;

    expect(flow.noFile).toBe(false);
    expect(flow.reviewing).toBe(true);

    second.resolve(plan('fresh'));
    await fresh;

    expect(flow.stage).toBe('review');
    expect(flow.plan?.planId).toBe('fresh');
  });
});
