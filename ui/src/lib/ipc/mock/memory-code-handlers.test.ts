import { afterEach, describe, expect, it, vi } from 'vitest';

import type { CodeBuildEvent } from '../bindings/CodeBuildEvent';
import { codeScenarioFromLocation, memoryCodeHandlers, type CodeScenario } from './memory-code-handlers';

afterEach(() => {
  vi.useRealTimers();
});

function listened(scenario: CodeScenario): CodeBuildEvent[] {
  vi.useFakeTimers();
  const events: CodeBuildEvent[] = [];
  memoryCodeHandlers(() => scenario, 10).memory_code_builds_listen({ onEvent: (event) => events.push(event) });
  vi.advanceTimersByTime(1000);

  return events;
}

describe('code index mock', () => {
  it('starts on from the default and remembers the choice as a file', async () => {
    const handlers = memoryCodeHandlers(() => 'ready');

    expect(await handlers.memory_code_index_get(undefined)).toEqual({ enabled: true, source: 'default' });
    expect(await handlers.memory_code_index_set({ on: false })).toEqual({ enabled: false, source: 'file' });
    expect(await handlers.memory_code_index_get(undefined)).toEqual({ enabled: false, source: 'file' });
  });

  it('is unsupported for ?code=old and reads the scenario from the location', () => {
    const handlers = memoryCodeHandlers(() => 'old');

    expect(() => handlers.memory_code_index_get(undefined)).toThrowError(/code index/);
    expect(codeScenarioFromLocation('?code=bulk')).toBe('bulk');
    expect(codeScenarioFromLocation('?code=nonsense')).toBe('ready');
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
