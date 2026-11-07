import { describe, expect, it } from 'vitest';

import { chipOf, dialOf, gaugeOptions, limitPace, limitView, resetText, tooEarly } from './limit-dial';
import { formatClock } from './limits';
import type { LimitWindow } from './models';

const HOUR = 3_600_000;
const NOW = Date.parse('2026-10-09T12:00:00Z');

/** A 5-hour window that is half over, 40% used. */
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

const at = (hours: number) => new Date(NOW + hours * HOUR).toISOString();

describe('limitPace', () => {
  it('projects the run-out from the burn rate over the elapsed time', () => {
    // 2.5h of 5h elapsed with 50% used: the other half lasts another 2.5h, exactly to the reset.
    const even = limitPace(window({ usedPercent: 50 }), NOW);

    expect(even?.projected).toBeCloseTo(1);
    expect(even?.outAt).toBeCloseTo(NOW + 2.5 * HOUR);

    // 80% in the same time runs out after 0.625h, at 1.6 times the whole window.
    const fast = limitPace(window({ usedPercent: 80 }), NOW);

    expect(fast?.projected).toBeCloseTo(1.6);
    expect(fast?.outAt).toBeCloseTo(NOW + 0.625 * HOUR);
  });

  it('has none when the use is not enough to tell, or there is nothing to measure it against', () => {
    expect(limitPace(window({ resetsAt: at(4.9), usedPercent: 30 }), NOW)).toBeNull();
    expect(limitPace(window({ usedPercent: 1 }), NOW)).toBeNull();
    expect(limitPace(window({ usedPercent: null, resetsAt: null }), NOW)).toBeNull();
    expect(limitPace(window({ resetsAt: null }), NOW)).toBeNull();
    expect(limitPace(window({ resetsAt: at(-1) }), NOW)).toBeNull();
    // A provider's own window name has no length.
    expect(limitPace(window({ window: 'monthly' }), NOW)).toBeNull();
  });
});

describe('the chip', () => {
  const chip = (overrides: Partial<LimitWindow>) => {
    const w = window(overrides);

    return chipOf(w, limitPace(w, NOW), NOW);
  };

  it('says a limit is on track, or when it ends', () => {
    expect(chip({ usedPercent: 40 })).toMatchObject({ kind: 'ok', text: 'On track' });
    expect(chip({ usedPercent: 80 })).toEqual({ kind: 'out', text: `Ends ${formatClock(NOW + 0.625 * HOUR, NOW)}` });
  });

  it('says so when nothing is used, the window has just begun, or the limit is used up', () => {
    expect(chip({ usedPercent: 1 })).toMatchObject({ kind: 'idle', text: 'Not used yet' });
    expect(chip({ resetsAt: at(4.9), usedPercent: 30 })).toMatchObject({ kind: 'idle', text: 'Too early' });
    expect(chip({ usedPercent: 100 })).toMatchObject({ kind: 'out', text: 'Used up' });
  });

  it('says a window is resetting in its last seconds, whatever the pace was', () => {
    expect(chip({ resetsAt: new Date(NOW + 10_000).toISOString(), usedPercent: 90 })).toMatchObject({ kind: 'idle', text: 'Resetting' });
  });

  it('treats a window that has reset since the reading as not used yet', () => {
    expect(chip({ usedPercent: null, resetsAt: null })).toMatchObject({ kind: 'idle', text: 'Not used yet' });
  });

  it('has none for a used window with no length to measure the pace against', () => {
    expect(chip({ window: 'monthly', usedPercent: 33 })).toBeNull();
    expect(tooEarly(window({ window: 'monthly', resetsAt: at(1) }), NOW)).toBe(false);
  });
});

describe('dialOf', () => {
  const fast = window({ usedPercent: 40, resetsAt: at(3.5) });
  const pace = limitPace(fast, NOW);

  it('draws the share used, with a pale stretch on to where the use will have got by the reset', () => {
    const dial = dialOf(fast, pace, 'used', true);

    // 1.5h of 5h elapsed with 40% used: heading for 133%, which the scale stops at its end.
    expect(dial.value).toBeCloseTo(0.4);
    expect(dial.solid).toBeCloseTo(0.4);
    expect(dial.ahead).toEqual({ from: expect.closeTo(0.4), to: 1 });
  });

  it('inverts the value, the solid part and the pale stretch when counting down', () => {
    const slow = window({ usedPercent: 20, resetsAt: at(2.5) });
    const dial = dialOf(slow, limitPace(slow, NOW), 'left', true);

    // 20% used at half time projects to 40%: 60% will be left, 80% is left now.
    expect(dial.value).toBeCloseTo(0.8);
    expect(dial.solid).toBeCloseTo(0.6);
    expect(dial.ahead).toEqual({ from: expect.closeTo(0.6), to: expect.closeTo(0.8) });
  });

  it('draws no pale stretch with the forecast off, or without a pace', () => {
    expect(dialOf(fast, pace, 'used', false).ahead).toBeNull();
    expect(dialOf(fast, pace, 'left', false)).toMatchObject({ value: expect.closeTo(0.6), solid: expect.closeTo(0.6), ahead: null });
    expect(dialOf(window({ window: 'monthly' }), null, 'used', true).ahead).toBeNull();
  });

  it('reads a window that has reset as empty when counting up and whole when counting down', () => {
    const reset = window({ usedPercent: null, resetsAt: null });

    expect(dialOf(reset, null, 'used', true).value).toBe(0);
    expect(dialOf(reset, null, 'left', true).value).toBe(1);
  });
});

describe('resetText', () => {
  it('counts down to the reset', () => {
    expect(resetText(window(), NOW)).toBe('resets in 2h 30m');
  });

  it('says a window has reset when the reading is gone or the time has passed, and when none was given', () => {
    expect(resetText(window({ usedPercent: null, resetsAt: null }), NOW)).toBe('has reset since last seen');
    expect(resetText(window({ resetsAt: at(-1) }), NOW)).toBe('has reset since last seen');
    expect(resetText(window({ resetsAt: null }), NOW)).toBe('no reset time given');
  });
});

describe('limitView', () => {
  it('names what the chart shows for a screen reader: subject, window, value, chip and reset', () => {
    const view = limitView(window({ usedPercent: 40 }), 'Claude Max', 'used', true, NOW);

    expect(view.percent).toBe(40);
    expect(view.label).toBe('Claude Max 5-hour limit: 40% used, On track, resets in 2h 30m');
  });

  it('reads the share left in the same words when counting down', () => {
    const view = limitView(window({ usedPercent: 40 }), 'Claude Max', 'left', true, NOW);

    expect(view.percent).toBe(60);
    expect(view.label).toBe('Claude Max 5-hour limit: 60% left, On track, resets in 2h 30m');
  });
});

describe('gaugeOptions', () => {
  it('offers the seven styles sorted by their name', () => {
    expect(gaugeOptions().map((option) => option.label)).toEqual(['Bars', 'Battery', 'Segmented arc', 'Tachometer', 'Thermometer', 'VU meter', 'Waffle']);
    expect(gaugeOptions().map((option) => option.value)).toEqual(['bars', 'battery', 'segments', 'tach', 'thermo', 'vu', 'waffle']);
  });
});
