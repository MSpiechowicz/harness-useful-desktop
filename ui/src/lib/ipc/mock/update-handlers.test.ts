import { describe, expect, it } from 'vitest';

import type { UpdateProgress } from '../bindings/UpdateProgress';
import { scenarioFromLocation, updateHandlers, UPDATE_SCENARIOS, type UpdateScenario } from './update-handlers';

function handlers(scenario: UpdateScenario) {
  return updateHandlers(() => 0, () => scenario);
}

describe('mock update backend', () => {
  it('announces an update with notes in the available scenario and nothing when current', async () => {
    expect(await handlers('available').update_check(undefined)).toMatchObject({ version: expect.any(String), notes: expect.stringContaining('0.2.0') });
    expect(await handlers('current').update_check(undefined)).toBeNull();
  });

  it.each([
    ['unconfigured', 'unsupported'],
    ['network', 'timeout'],
  ] as const)('fails the check as %s with the %s code', async (scenario, code) => {
    await expect(handlers(scenario).update_check(undefined)).rejects.toMatchObject({ code });
  });

  it('refuses an install that no check preceded', async () => {
    await expect(handlers('available').update_install({ onProgress: () => {} })).rejects.toMatchObject({ code: 'notFound' });
  });

  it('streams the download to the end, then installing, then done', async () => {
    const mock = handlers('available');
    const seen: UpdateProgress[] = [];
    await mock.update_check(undefined);

    await mock.update_install({ onProgress: (progress) => seen.push(progress) });

    expect(seen.at(-1)).toMatchObject({ phase: 'done' });
    expect(seen.at(-2)).toMatchObject({ phase: 'installing' });
    expect(seen.filter((progress) => progress.phase === 'downloading').map((progress) => progress.downloaded)).toEqual(
      [...seen.filter((progress) => progress.phase === 'downloading').map((progress) => progress.downloaded)].sort((a, b) => a - b),
    );
  });

  it('fails verification only after most of the download was seen', async () => {
    const mock = handlers('forbidden');
    const seen: UpdateProgress[] = [];
    await mock.update_check(undefined);

    await expect(mock.update_install({ onProgress: (progress) => seen.push(progress) })).rejects.toMatchObject({ code: 'forbidden' });
    expect(seen.length).toBeGreaterThan(10);
    expect(seen.every((progress) => progress.phase === 'downloading')).toBe(true);
  });

  it('picks the scenario from the page address and falls back to an available update', () => {
    for (const name of UPDATE_SCENARIOS) {
      expect(scenarioFromLocation(`?update=${name}`)).toBe(name);
    }

    expect(scenarioFromLocation('?update=nonsense')).toBe('available');
    expect(scenarioFromLocation('')).toBe('available');
  });
});
