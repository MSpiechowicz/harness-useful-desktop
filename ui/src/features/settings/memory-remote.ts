import type { MessageKey } from '$lib/i18n/i18n.svelte';
import type { MemoryRemoteKind } from '$lib/ipc/bindings/MemoryRemoteKind';

export const REMOTE_KINDS: readonly MemoryRemoteKind[] = ['postgres', 'libsql', 'mongodb'];

/** The URL schemes each database accepts; the backend checks again and decides. */
const SCHEMES: Record<MemoryRemoteKind, readonly string[]> = {
  postgres: ['postgres://', 'postgresql://'],
  libsql: ['libsql://', 'https://', 'http://'],
  mongodb: ['mongodb://', 'mongodb+srv://'],
};

export function urlMatchesKind(kind: MemoryRemoteKind, url: string): boolean {
  const lower = url.trim().toLowerCase();

  return SCHEMES[kind].some((scheme) => lower.startsWith(scheme) && lower.length > scheme.length);
}

/** Only libSQL (Turso) databases take a separate auth token. */
export function takesToken(kind: MemoryRemoteKind): boolean {
  return kind === 'libsql';
}

export const KIND_LABELS = {
  postgres: 'settings.memoryRemote.kind.postgres',
  libsql: 'settings.memoryRemote.kind.libsql',
  mongodb: 'settings.memoryRemote.kind.mongodb',
} as const satisfies Record<MemoryRemoteKind, MessageKey>;

export const URL_PLACEHOLDERS = {
  postgres: 'settings.memoryRemote.url.placeholder.postgres',
  libsql: 'settings.memoryRemote.url.placeholder.libsql',
  mongodb: 'settings.memoryRemote.url.placeholder.mongodb',
} as const satisfies Record<MemoryRemoteKind, MessageKey>;

export const URL_PROBLEMS = {
  postgres: 'settings.memoryRemote.url.invalid.postgres',
  libsql: 'settings.memoryRemote.url.invalid.libsql',
  mongodb: 'settings.memoryRemote.url.invalid.mongodb',
} as const satisfies Record<MemoryRemoteKind, MessageKey>;
