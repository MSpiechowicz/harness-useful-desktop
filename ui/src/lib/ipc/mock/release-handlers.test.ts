import { describe, expect, it } from 'vitest';

import { releaseHandlers } from './release-handlers';
import { MockState } from './state';

describe('mock companion releases', () => {
  it('flags the skills as behind and the dashboard as current', async () => {
    const mock = releaseHandlers(new MockState());

    const releases = await mock.ecosystem_releases({
      installed: [
        { id: 'dashboard', version: '1.22.1' },
        { id: 'skills', version: '0.2.36' },
      ],
    });

    expect(releases).toEqual([
      { id: 'dashboard', latestVersion: '1.22.1', updateAvailable: false },
      { id: 'skills', latestVersion: '0.2.40', updateAvailable: true },
    ]);
  });

  it('answers only for companions with a version', async () => {
    const mock = releaseHandlers(new MockState());

    expect(await mock.ecosystem_releases({ installed: [{ id: 'sidebar', version: ' ' }] })).toEqual([]);
  });

  it('looks nothing up while companion checks are off', async () => {
    const state = new MockState();
    state.save({ ...state.snapshot(), checkCompanionUpdates: false });

    expect(await releaseHandlers(state).ecosystem_releases({ installed: [{ id: 'skills', version: '0.2.36' }] })).toEqual([]);
  });
});
