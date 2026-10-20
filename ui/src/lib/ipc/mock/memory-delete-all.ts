import type { MemoryDeleteAll } from '../bindings/MemoryDeleteAll';
import type { MemoryScriptsAvailability } from '../bindings/MemoryScriptsAvailability';
import type { MemoryRepos } from '../bindings/MemoryRepos';
import { IpcFailure } from '../transport';

/** Rejects the way the real backend does while the server cannot be used: missing is `notFound`, too old `unsupported`, failing `process`. */
export function requireServer(availability: MemoryScriptsAvailability): void {
  switch (availability.kind) {
    case 'available':
      return;
    case 'notInstalled':
      throw IpcFailure.from({ code: 'notFound', message: 'executable harness-memory was not found' });
    case 'outdated':
      throw IpcFailure.from({ code: 'unsupported', message: 'this harness-memory predates `delete --all`; update Harness Memory' });
    case 'failed':
      throw IpcFailure.from({ code: 'process', message: availability.message });
  }
}

/** What `memory_notes_delete_all` and `memory_scripts_delete_all` answer for the entries they deleted, and the `kept` team notes that cannot be deleted from here. */
function deletedAll(gone: { place: 'team' | 'local' }[], kept = 0): MemoryDeleteAll {
  return { deleted: gone.length, team: gone.filter((entry) => entry.place === 'team').length, kept };
}

/** An entry a delete-all can remove: a team entry that is read-only here stays. */
interface Deletable {
  place: 'team' | 'local';
  scope: 'repo' | 'global';
  readOnly: boolean;
}

/**
 * What `memory_notes_delete_all` and `memory_scripts_delete_all` remove from `visible` (what the list shows for the reach): `gone` is
 * every entry of `scope` (any when unset) that is not read-only, and `answer` is what the command replies. Every repository cannot be
 * deleted at once.
 */
export function deletion<T extends Deletable>(visible: T[], repos: MemoryRepos | undefined, scope: Deletable['scope'] | undefined): { gone: T[]; answer: MemoryDeleteAll } {
  if (repos?.kind === 'all') {
    throw IpcFailure.from({ code: 'invalidInput', message: 'every repository cannot be deleted at once; pick one' });
  }

  const all = visible.filter((candidate) => scope === undefined || candidate.scope === scope);
  const gone = all.filter((candidate) => !candidate.readOnly);

  return { gone, answer: deletedAll(gone, all.length - gone.length) };
}
