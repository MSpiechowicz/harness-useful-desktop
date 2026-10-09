import { readJson, writeJson } from '$lib/utils/persisted';

const KEY = 'harness.paletteRecents';
const MAX_RECENTS = 5;

function isIdList(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === 'string');
}

/** The palette entries run most recently, newest first. Ids only: no content is stored. */
export class PaletteRecents {
  ids = $state<string[]>(readJson(KEY, [], isIdList));

  /** Entries that only exist for a moment (a session, a file) would go stale; only stable ids are kept. */
  record(id: string): void {
    this.ids = [id, ...this.ids.filter((existing) => existing !== id)].slice(0, MAX_RECENTS);
    writeJson(KEY, this.ids);
  }

  clear(): void {
    this.ids = [];
    writeJson(KEY, this.ids);
  }
}

export const paletteRecents = new PaletteRecents();
