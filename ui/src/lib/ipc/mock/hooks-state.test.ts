import { describe, expect, it } from 'vitest';

import { type HookScenario, MockHooks } from './hooks-state';

const hooks = (scenario: HookScenario = 'ok') => new MockHooks(() => scenario, () => true);

describe('the mock family catalog', () => {
  it('lists the steps in order with their positions, and the follow-up as no step', () => {
    const entries = hooks().family();

    expect(entries.map((entry) => [entry.seed, entry.step])).toEqual([
      ['dashboard-limits', 10],
      ['branch-sync', 20],
      ['useful-skills', 30],
      ['memory-first', 40],
      ['knowledge-first', 50],
      ['code-graph-first', 60],
      ['memory-save', null],
    ]);
    expect(entries.every((entry) => !entry.retired)).toBe(true);
  });

  it('lists a merged hook as a retired update only while its file is left', () => {
    const state = hooks('merged');
    const merged = () => state.family().filter((entry) => entry.retired);

    expect(merged()).toMatchObject([{ seed: 'memory-search', status: 'outdated', stale: true, step: null }]);

    state.applyFamily(['memory-search']);

    expect(merged()).toEqual([]);
  });

  it('composes the first message of a chat as the intro, then the active steps numbered from 1, with the notes the app found in the memory step', () => {
    const context = hooks().sessionContext(null);

    expect(context).toContain('<hook name="Harness steps">');
    expect(context).toMatch(/<hook name="Harness Memory">1\. [^]*<harness-memory-recall>[^]*<\/harness-memory-recall><\/hook>/);
    expect(hooks('empty').sessionContext(null)).toBe('');
  });
});
