import { vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { MockMemoryGuards } from '$lib/ipc/mock/memory-guards-handlers';

const OWN = ['memory_guards_get', 'memory_guards_update', 'memory_saving_root'] as const;

/**
 * Answers the global guards from a mock of its own, so a test can make a read fail, hold a read back or set `migrationPending` and see
 * the app follow. A folder in `roots` has that saving root (null: none), any other is its own root. Everything else still goes to the
 * app's mock backend. Undone by `vi.restoreAllMocks()`.
 */
export function guardsBackend(roots: Record<string, string | null> = {}) {
  const guards = new MockMemoryGuards(() => 'empty');
  const handlers = guards.handlers() as unknown as Record<string, (args: unknown) => unknown>;
  const real = ipc.invoke.bind(ipc);
  const calls: CommandName[] = [];
  let gate: Promise<void> | null = null;

  vi.spyOn(ipc, 'invoke').mockImplementation((async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    if (!(OWN as readonly string[]).includes(name)) {
      return real(name, args);
    }

    calls.push(name);

    if (name === 'memory_guards_get' && gate) {
      await gate;
    }

    if (name === 'memory_saving_root') {
      const folder = (args as { folder: string }).folder;

      return (folder in roots ? roots[folder] : folder) as CommandResult<K>;
    }

    return handlers[name](args) as CommandResult<K>;
  }) as typeof ipc.invoke);

  return {
    guards,
    roots,
    count: (name: CommandName) => calls.filter((call) => call === name).length,
    /** Reads of the guards wait until the returned function is called. */
    hold(): () => void {
      let release = () => {};
      gate = new Promise<void>((resolve) => (release = resolve));

      return () => {
        gate = null;
        release();
      };
    },
  };
}
