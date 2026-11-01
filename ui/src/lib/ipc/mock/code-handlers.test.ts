import { afterEach, describe, expect, it, vi } from 'vitest';

import type { CodeBuildEvent } from '../bindings/CodeBuildEvent';
import { codeScenarioFromLocation, codeHandlers, type CodeScenario } from './code-handlers';

afterEach(() => {
  vi.useRealTimers();
});

function listened(scenario: CodeScenario): CodeBuildEvent[] {
  vi.useFakeTimers();
  const events: CodeBuildEvent[] = [];
  codeHandlers(() => scenario, 10).code_builds_listen({ onEvent: (event) => events.push(event) });
  vi.advanceTimersByTime(1000);

  return events;
}

describe('code index mock', () => {
  it('starts on from the default and remembers the choice as a file', async () => {
    const handlers = codeHandlers(() => 'ready');

    expect(await handlers.code_index_get(undefined)).toEqual({ enabled: true, source: 'default' });
    expect(await handlers.code_index_set({ on: false })).toEqual({ enabled: false, source: 'file' });
    expect(await handlers.code_index_get(undefined)).toEqual({ enabled: false, source: 'file' });
  });

  it('is not found without Harness Code installed, but still follows the builds', () => {
    const handlers = codeHandlers(() => 'ready', 10, () => false);

    expect(() => handlers.code_index_get(undefined)).toThrowError(/not installed/);
    expect(() => handlers.code_index_set({ on: false })).toThrowError(/not installed/);
    expect(() => handlers.code_prepare({ project: '/home/demo/project' })).toThrowError(/not installed/);
    expect(handlers.code_builds_listen({ onEvent: () => {} })).toBeNull();
  });

  it('is unsupported for ?code=old and reads the scenario from the location', () => {
    const handlers = codeHandlers(() => 'old');

    expect(() => handlers.code_index_get(undefined)).toThrowError(/code index/);
    expect(() => handlers.code_prepare({ project: '/home/demo/project' })).toThrowError(/code index/);
    expect(codeScenarioFromLocation('?code=bulk')).toBe('bulk');
    expect(codeScenarioFromLocation('?code=nonsense')).toBe('ready');
  });

  it('accepts a project to prepare and answers at once with nothing', () => {
    const handlers = codeHandlers(() => 'ready');

    expect(handlers.code_prepare({ project: '/home/demo/project' })).toBeNull();
  });

  it('sends a first index as building steps and a ready end, and nothing in the default scenario', () => {
    const phases = listened('build').map((event) => event.phase);

    expect(phases).toEqual(['building', 'building', 'building', 'building', 'building', 'ready']);
    expect(listened('ready')).toEqual([]);
  });

  it('ends a failed bulk build with its reason and a cancelled or stale one without', () => {
    const failed = listened('bulk').at(-1);
    expect(failed).toMatchObject({ kind: 'bulk', phase: 'error' });
    expect(failed?.error).toMatch(/disk full/);

    expect(listened('cancelled').at(-1)).toMatchObject({ phase: 'cancelled', error: null });
    expect(listened('stale').at(-1)).toMatchObject({ phase: 'stale', error: null });
  });
});
