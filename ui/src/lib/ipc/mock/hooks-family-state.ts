import type { Hook } from '../bindings/Hook';
import type { HookFamilyEntry } from '../bindings/HookFamilyEntry';
import type { HookRequirement } from '../bindings/HookRequirement';
import type { HookSeed } from '../bindings/HookSeed';
import { CATALOG, type CatalogEntry, OLD_SAVE_TEXT, RETIRED } from './hooks-fixtures';

/** What the Hooks view reads of the family catalog: where each hook stands against the hook files on disk. */
export class MockFamily {
  constructor(
    /** The hook file of a family hook, by its seed. */
    private readonly fileOf: (seed: HookSeed) => Hook | undefined,
    /** Whether the member a hook needs is installed. */
    private readonly installed: (member: HookRequirement) => boolean,
  ) {}

  status(entry: CatalogEntry): HookFamilyEntry['status'] {
    const found = this.fileOf(entry.seed);

    if (!this.installed(entry.member)) {
      return 'needsMember';
    }

    if (!found) {
      return 'missing';
    }

    return found.action.type !== 'command' && found.action.text === entry.text ? 'installed' : 'outdated';
  }

  /** The catalog in step order, then the hooks it merged into others while an untouched file of them is left. */
  entries(): HookFamilyEntry[] {
    const merged = RETIRED.filter((entry) => this.fileOf(entry.seed) !== undefined);

    return [...CATALOG.map((entry) => this.current(entry)), ...merged.map((entry) => this.retired(entry))];
  }

  private current(entry: CatalogEntry): HookFamilyEntry {
    return {
      seed: entry.seed,
      member: entry.member,
      name: entry.name,
      event: entry.event,
      status: this.status(entry),
      hookId: this.fileOf(entry.seed)?.id ?? null,
      stale: this.isStale(entry),
      step: entry.step,
      retired: false,
    };
  }

  /** A merged hook is only an update that removes its file. */
  private retired(entry: CatalogEntry): HookFamilyEntry {
    return { seed: entry.seed, member: entry.member, name: entry.name, event: entry.event, status: 'outdated', hookId: this.fileOf(entry.seed)?.id ?? null, stale: true, step: null, retired: true };
  }

  /** An outdated family hook that still holds the earlier catalog text, word for word. */
  private isStale(entry: CatalogEntry): boolean {
    const found = this.fileOf(entry.seed);

    return this.status(entry) === 'outdated' && found?.action.type !== 'command' && found?.action.text === OLD_SAVE_TEXT;
  }
}
