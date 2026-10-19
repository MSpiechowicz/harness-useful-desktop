import type { MemoryDeleteAll } from '../bindings/MemoryDeleteAll';
import type { MemoryScriptsAvailability } from '../bindings/MemoryScriptsAvailability';
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
export function deletedAll(gone: { place: 'team' | 'local' }[], kept = 0): MemoryDeleteAll {
  return { deleted: gone.length, team: gone.filter((entry) => entry.place === 'team').length, kept };
}
