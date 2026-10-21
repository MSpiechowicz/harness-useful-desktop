import type { HarnessId } from './bindings/HarnessId';

/**
 * Every harness id the backend can send. A `Record` over `HarnessId` has to list each variant, so a harness added
 * to the generated type fails the type check here until it is listed, and is never silently treated as unknown.
 */
const KNOWN: Record<HarnessId, true> = { omp: true, pi: true, opencode: true };

export const HARNESS_IDS = Object.keys(KNOWN) as HarnessId[];

/** Whether a stored or received value names a harness this build knows. */
export function isHarnessId(value: unknown): value is HarnessId {
  return typeof value === 'string' && Object.hasOwn(KNOWN, value);
}

/** OpenCode changed its configuration and tool names in its second major version, so a few texts differ by generation. */
export type OpenCodeGeneration = 1 | 2;

/** The generation a detected OpenCode version belongs to: major 2 and above is 2, anything older or unreadable is 1. */
export function openCodeGeneration(version: string | null | undefined): OpenCodeGeneration {
  const major = Number.parseInt(version?.match(/\d+/)?.[0] ?? '', 10);

  return major >= 2 ? 2 : 1;
}
