import { fireEvent, screen } from '@testing-library/svelte';

import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { sampleScripts } from '$lib/ipc/mock/memory-scripts-fixtures';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { fakeMemory, showMemory, showNotes, withTeamDatabase } from './memory-test-support';

/** What a page read during an outage carries: the real answer, flagged `offline`, with other counts of what is saved on this computer when a test sets them. */
export interface Outage {
  down: boolean;
  counts: { repo: number; global: number } | null;
}

/** Lets a test take the remote database down and up. While it is down the lists answer as from this computer's copy. */
export function outageOf(invoke: ReturnType<typeof fakeMemory>): Outage {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;
  const outage: Outage = { down: false, counts: null };

  invoke.mockImplementation((async (name: CommandName, args: never) => {
    const answer = await real(name as never, args);

    if (outage.down && (name === 'memory_notes_list' || name === 'memory_scripts_list')) {
      return { ...(answer as object), offline: true, ...(outage.counts ? { localCounts: outage.counts } : {}) };
    }

    return answer;
  }) as typeof ipc.invoke);

  return outage;
}

/** Changes what the fake says the installed Harness Memory and its settings are, over what it answered. */
export function withRemote(invoke: ReturnType<typeof fakeMemory>, patch: Partial<MemoryRemote>): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;

  invoke.mockImplementation((async (name: CommandName, args: never) => {
    const answer = await real(name as never, args);

    return name === 'memory_remote_get' ? { ...(answer as object), ...patch } : answer;
  }) as typeof ipc.invoke);
}

/** The Settings switches of the two copies. */
export function withCopyPrefs(toRemote: boolean, toLocal: boolean): void {
  prefs.value = samplePrefs({ motion: 'off', memoryCopyToRemote: toRemote, memoryCopyToLocal: toLocal });
}

/** Two scripts saved on this computer next to the remote ones. */
export const localScripts = (): ScriptDetail[] => [100, 101].map((id) => ({ ...sampleScripts()[1], id, name: `local-script-${id}`, scope: 'global' as const, repo: null, place: 'local' as const }));

/** The remote scripts of the fixtures and two saved here. */
export const scriptsWithLocal = (): ScriptDetail[] => [...sampleScripts().filter((script) => script.place === 'team'), ...localScripts()];

/** Which view a test runs in: the Notes view with its notes read from both places, or the Scripts view with its scripts. */
export interface SyncView {
  name: 'notes' | 'scripts';
  /** Opens the view with its entries kept in `place` (read from the remote and this computer together unless a test says otherwise), and the remote settings changed by `remote`. */
  open: (remote?: Partial<MemoryRemote>, place?: MemoryNotesRouting) => Promise<{ invoke: ReturnType<typeof fakeMemory>; outage: Outage }>;
}

export const SYNC_VIEWS: SyncView[] = [
  {
    name: 'notes',
    open: async (remote = {}, place = 'remoteLocal') => {
      const invoke = fakeMemory();
      withTeamDatabase(invoke, { repositoryNotes: place, globalNotes: place });
      withRemote(invoke, remote);
      const outage = outageOf(invoke);
      await showNotes();

      return { invoke, outage };
    },
  },
  {
    name: 'scripts',
    open: async (remote = {}, place = 'remoteLocal') => {
      const invoke = fakeMemory({ kind: 'available' }, scriptsWithLocal());
      withTeamDatabase(invoke, { repositoryNotes: 'local', globalNotes: 'local', scripts: place });
      withRemote(invoke, remote);
      const outage = outageOf(invoke);
      await showMemory();

      return { invoke, outage };
    },
  },
];

export const syncButton = () => screen.queryByRole('button', { name: 'Synchronize…' });

/** Opens the Sync dialog from the button of the view. */
export async function openSync(): Promise<void> {
  await fireEvent.click(await screen.findByRole('button', { name: 'Synchronize…' }));
  await screen.findByRole('dialog');
}
