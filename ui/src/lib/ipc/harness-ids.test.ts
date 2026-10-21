import { describe, expect, it } from 'vitest';

import { fixtures } from './mock/fixtures';
import { HARNESS_IDS, isHarnessId, openCodeGeneration } from './harness-ids';

describe('the known harness ids', () => {
  it('cover every harness the backend lists, so none is dropped as unknown', () => {
    expect([...HARNESS_IDS].sort()).toEqual(fixtures.harnesses.map((summary) => summary.info.id).sort());
  });

  it('accept those ids only', () => {
    expect(isHarnessId('opencode')).toBe(true);
    expect(isHarnessId('gemini')).toBe(false);
    expect(isHarnessId('toString')).toBe(false);
    expect(isHarnessId(7)).toBe(false);
  });
});

describe('the OpenCode generation of a detected version', () => {
  it.each([
    ['1.19.3', 1],
    ['1.18.0', 1],
    ['2.1.2', 2],
    ['v2.0.0', 2],
    ['opencode v3.4.0', 2],
    ['10.0.0', 2],
  ] as const)('reads %j as generation %i', (version, generation) => {
    expect(openCodeGeneration(version)).toBe(generation);
  });

  it.each([null, undefined, '', 'unknown'])('reads %j as generation 1', (version) => {
    expect(openCodeGeneration(version)).toBe(1);
  });
});
