import type { KnowledgeId } from './bindings/KnowledgeId';

/**
 * A document or chunk id as the server gave it: a number, or a `local:N` string for this computer's store when both places are in use.
 * It goes back to the server exactly as it came, so a local id is never parsed into a number.
 */
export type { KnowledgeId };

/** The id as a map key or element id. A number and its string never collide: local ids carry their prefix. */
export function idKey(id: KnowledgeId): string {
  return String(id);
}

/** The id behind a key made by {@link idKey}. */
export function idFromKey(key: string): KnowledgeId {
  return /^\d+$/.test(key) ? Number(key) : key;
}
