import { describe, expect, it } from 'vitest';

import { formatAge, formatClock, groupLimits, leftShare, leftTone, paceOf, planLabel, planName, providerHarness, providerName, recentGroups, runsOutBeforeReset, windowLengthMs, windowName } from './limits';
import type { LimitWindow } from './models';

const HOUR = 3_600_000;
const NOW = Date.parse('2026-10-09T12:00:00Z');

function window(overrides: Partial<LimitWindow> = {}): LimitWindow {
  return {
    provider: 'claude',
    plan: 'max',
    window: '5h',
    scope: null,
    usedPercent: 40,
    resetsAt: new Date(NOW + 2.5 * HOUR).toISOString(),
    ageMinutes: 3,
    status: 'fresh',
    ...overrides,
  };
}

describe('limit names', () => {
  it('names a window by its length and scope', () => {
    expect(windowName('5h', null)).toBe('5-hour limit');
    expect(windowName('7d', null)).toBe('Weekly limit');
    expect(windowName('7d', 'Fable')).toBe('Weekly limit · Fable');
    expect(windowName('1d', null)).toBe('Daily limit');
    expect(windowName('30d', null)).toBe('30-day limit');
    expect(windowName('premium', null)).toBe('premium');
  });

  it('reads the window length from its name', () => {
    expect(windowLengthMs('5h')).toBe(5 * HOUR);
    expect(windowLengthMs('7d')).toBe(7 * 24 * HOUR);
    expect(windowLengthMs('monthly')).toBeNull();
  });

  it('writes provider and plan names the way the dashboard does', () => {
    expect(providerName('claude')).toBe('Claude');
    expect(providerName('some-new_ai')).toBe('Some New Ai');
    expect(planName('prolite')).toBe('Pro Lite');
    expect(planName('max')).toBe('Max');
  });
});

describe('what is left', () => {
  it('turns used percent into the share left and treats a reset window as whole', () => {
    expect(leftShare(32)).toBeCloseTo(0.68);
    expect(leftShare(100)).toBe(0);
    expect(leftShare(120)).toBe(0);
    expect(leftShare(null)).toBe(1);
  });

  it('warns as little is left', () => {
    expect(leftTone(0.67)).toBe('ok');
    expect(leftTone(0.25)).toBe('ok');
    expect(leftTone(0.2)).toBe('low');
    expect(leftTone(0.05)).toBe('critical');
  });
});

describe('pace', () => {
  it('projects the run-out from the burn rate over the elapsed time', () => {
    // 2.5h of 5h elapsed with 50% used: the other half lasts another 2.5h, exactly to the reset.
    const even = paceOf(window({ usedPercent: 50 }), NOW);

    expect(even?.timeLeft).toBeCloseTo(0.5);
    expect(even?.outAt).toBeCloseTo(NOW + 2.5 * HOUR);

    // 80% used in the same time runs out after 0.625h.
    const fast = paceOf(window({ usedPercent: 80 }), NOW);

    expect(fast?.outAt).toBeCloseTo(NOW + 0.625 * HOUR);
    expect(runsOutBeforeReset(fast!, window().resetsAt)).toBe(true);
  });

  it('lasts until the reset when use is below the time elapsed', () => {
    const slow = paceOf(window({ usedPercent: 10 }), NOW);

    expect(slow?.outAt).toBeGreaterThan(NOW + 2.5 * HOUR);
    expect(runsOutBeforeReset(slow!, window().resetsAt)).toBe(false);
  });

  it('only marks the time left when it is too early or too little used to project', () => {
    const early = paceOf(window({ resetsAt: new Date(NOW + 4.9 * HOUR).toISOString(), usedPercent: 30 }), NOW);
    const idle = paceOf(window({ usedPercent: 1 }), NOW);

    expect(early?.outAt).toBeNull();
    expect(early?.timeLeft).toBeCloseTo(0.98);
    expect(idle?.outAt).toBeNull();
    expect(idle?.timeLeft).toBeCloseTo(0.5);
    expect(runsOutBeforeReset(idle!, window().resetsAt)).toBe(false);
  });

  it('gives no pace without a current reading, a reset time, or a known window length', () => {
    expect(paceOf(window({ usedPercent: null, resetsAt: null }), NOW)).toBeNull();
    expect(paceOf(window({ resetsAt: null }), NOW)).toBeNull();
    expect(paceOf(window({ resetsAt: new Date(NOW - HOUR).toISOString() }), NOW)).toBeNull();
    expect(paceOf(window({ window: 'monthly' }), NOW)).toBeNull();
  });

  it('measures a weekly window against seven days', () => {
    const week = paceOf(window({ window: '7d', resetsAt: new Date(NOW + 84 * HOUR).toISOString(), usedPercent: 25 }), NOW);

    expect(week?.timeLeft).toBeCloseTo(0.5);
    expect(week?.outAt).toBeGreaterThan(NOW + 84 * HOUR);
  });
});

describe('clock and age', () => {
  it('adds the weekday only for a time more than a day out', () => {
    expect(formatClock(NOW + 2 * HOUR, NOW)).toMatch(/^\d{2}:\d{2}$/);
    expect(formatClock(NOW + 40 * HOUR, NOW)).toMatch(/^[A-Z][a-z]{2} \d{2}:\d{2}$/);
  });

  it('writes how old a reading is', () => {
    expect(formatAge(0)).toBe('1m ago');
    expect(formatAge(45)).toBe('45m ago');
    expect(formatAge(380)).toBe('6h ago');
    expect(formatAge(3000)).toBe('2d ago');
  });
});

describe('grouping', () => {
  it('groups windows by provider and plan in order of appearance', () => {
    const groups = groupLimits([window(), window({ window: '7d' }), window({ provider: 'codex', plan: 'pro' }), window({ provider: 'codex', plan: 'plus' })]);

    expect(groups.map((group) => [group.provider, group.plan, group.windows.length])).toEqual([
      ['claude', 'max', 2],
      ['codex', 'pro', 1],
      ['codex', 'plus', 1],
    ]);
  });

  it('dates a group only when none of its readings is fresh', () => {
    const fresh = groupLimits([window(), window({ status: 'stale', ageMinutes: 380 })]);
    const stale = groupLimits([window({ status: 'stale', ageMinutes: 380 }), window({ status: 'reset since reading', ageMinutes: 200 })]);

    expect(fresh[0]?.staleMinutes).toBeNull();
    expect(stale[0]?.staleMinutes).toBe(200);
  });
});

describe('recentGroups', () => {
  it('keeps the plans in use, not the ones the dashboard merely refreshed', () => {
    const groups = groupLimits([
      window(),
      window({ provider: 'codex', plan: 'prolite', usedPercent: null, status: 'reset since reading', ageMinutes: 40_000 }),
      window({ provider: 'codex', plan: 'free', usedPercent: 0 }),
      window({ provider: 'opencode-go', plan: 'OpenCode Go', usedPercent: 1 }),
    ]);

    expect(recentGroups(groups).map((group) => [group.provider, group.plan])).toEqual([
      ['claude', 'max'],
      ['opencode-go', 'OpenCode Go'],
    ]);
  });

  it('falls back to the most recently read plans when none is in use', () => {
    const groups = groupLimits([
      window({ provider: 'codex', plan: 'free', usedPercent: 0, status: 'stale', ageMinutes: 600 }),
      window({ provider: 'codex', plan: 'pro', usedPercent: 0, status: 'stale', ageMinutes: 30 }),
    ]);

    expect(recentGroups(groups).map((group) => group.plan)).toEqual(['pro']);
  });
});

describe('OpenCode Go', () => {
  it('names the subscription once and wears the OpenCode color', () => {
    expect(providerName('opencode-go')).toBe('OpenCode Go');
    expect(planLabel('opencode-go', 'OpenCode Go')).toBeNull();
    expect(planLabel('claude', 'max')).toBe('Max');
    expect(planLabel('claude', null)).toBeNull();
    expect(providerHarness('opencode-go')).toBe('opencode');
    expect(providerHarness('claude')).toBe('claude');
  });
});
