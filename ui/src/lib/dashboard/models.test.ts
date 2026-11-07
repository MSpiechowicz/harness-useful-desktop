import { describe, expect, it } from 'vitest';

import type { DashboardResult } from '$lib/ipc/bindings/DashboardResult';
import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';

import { readLimits } from './models';

function limits(...windows: Record<string, JsonValue>[]): DashboardResult {
  return { tool: 'limits', data: { windows: windows.map((window) => ({ provider: 'claude', plan: 'max', window: '5h', ...window })) }, text: null, isError: false };
}

describe('readLimits', () => {
  it('reads the share used as the dashboard sends it', () => {
    expect(readLimits(limits({ usedPercent: 43 }))[0]?.usedPercent).toBe(43);
  });

  it('turns a window that reports the share left into the share used', () => {
    // The dashboard's MCP counts down when its own setting is Left: the app draws the same either way.
    const [window] = readLimits(limits({ leftPercent: 57 }));

    expect(window?.usedPercent).toBe(43);
  });

  it('keeps the share used when both are sent, and clamps a share left outside 0..100', () => {
    const windows = readLimits(limits({ usedPercent: 10, leftPercent: 70 }, { leftPercent: 120 }, { leftPercent: -5 }));

    expect(windows.map((window) => window.usedPercent)).toEqual([10, 0, 100]);
  });

  it('has no share used for a window that has reset since the reading', () => {
    expect(readLimits(limits({ usedPercent: null }, {}))[0]?.usedPercent).toBeNull();
    expect(readLimits(limits({}))[0]?.usedPercent).toBeNull();
  });
});
