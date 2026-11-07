import { describe, expect, it } from 'vitest';

import { formatAge, formatClock, groupLimits, planLabel, planName, planSubject, providerHarness, providerName, recentGroups, shortWindowName, windowLengthMs, windowName } from './limits';
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

  it('names a window for a narrow gauge by its scope, else by its length', () => {
    expect(shortWindowName('7d', 'Fable')).toBe('Fable');
    expect(shortWindowName('5h', null)).toBe('5 hours');
    expect(shortWindowName('7d', null)).toBe('Week');
    expect(shortWindowName('1d', null)).toBe('Day');
    expect(shortWindowName('30d', null)).toBe('30 days');
    expect(shortWindowName('1h', null)).toBe('1 hour');
    expect(shortWindowName('monthly', null)).toBe('monthly');
  });

  it('writes provider and plan names the way the dashboard does', () => {
    expect(providerName('claude')).toBe('Claude');
    expect(providerName('some-new_ai')).toBe('Some New Ai');
    expect(planName('prolite')).toBe('Pro Lite');
    expect(planName('max')).toBe('Max');
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

  it('keeps every provider in use, one plan each', () => {
    const groups = groupLimits([window(), window({ provider: 'codex', plan: 'pro' }), window({ provider: 'codex', plan: 'free', usedPercent: 5 }), window({ provider: 'copilot', plan: 'business' }), window({ provider: 'opencode-go', plan: 'OpenCode Go' })]);

    expect(recentGroups(groups).map((group) => group.provider)).toEqual(['claude', 'codex', 'copilot', 'opencode-go']);
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

describe('names that are also Object properties', () => {
  it.each(['constructor', '__proto__', 'toString', 'valueOf'])('reads the provider and plan %s as plain text', (name) => {
    expect(typeof providerName(name)).toBe('string');
    expect(providerHarness(name)).toBe(name);
    expect(typeof planName(name)).toBe('string');
    expect(planSubject('claude', name)).toBe(`Claude ${planName(name)}`);
  });
});
