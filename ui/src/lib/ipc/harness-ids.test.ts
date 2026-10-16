import { describe, expect, it } from 'vitest';

import { fixtures } from './mock/fixtures';
import { HARNESS_IDS, isHarnessId } from './harness-ids';

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
