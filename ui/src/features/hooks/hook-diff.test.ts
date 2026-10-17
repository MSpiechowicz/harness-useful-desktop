import { describe, expect, it } from 'vitest';

import type { Hook } from '$lib/ipc/bindings/Hook';

import { changesOf } from './hook-diff';

const BASE: Hook = {
  id: 'a'.repeat(32),
  name: 'Run lint after edits',
  description: null,
  scope: 'project',
  project: '/work/app',
  event: 'toolEnd',
  matcher: 'edit',
  action: { type: 'command', command: 'bun run lint', timeoutSecs: null },
  enabled: true,
  requires: [],
  seed: null,
  createdAt: '2026-10-17T10:00:00Z',
  createdBy: 'agent',
  updatedAt: '2026-10-17T10:00:00Z',
  updatedBy: 'agent',
};

describe('changesOf', () => {
  it('lists only what differs from the approved version', () => {
    const waiting: Hook = { ...BASE, matcher: 'edit|write', action: { type: 'command', command: 'bun run lint --quiet', timeoutSecs: 20 } };

    expect(changesOf(waiting, BASE)).toEqual([
      { field: 'matcher', from: 'edit', to: 'edit|write' },
      { field: 'text', from: 'bun run lint', to: 'bun run lint --quiet' },
      { field: 'timeout', from: null, to: '20' },
    ]);
  });

  it('lists every part a never approved hook sets, and skips the empty ones', () => {
    expect(changesOf(BASE, null).map((change) => change.field)).toEqual(['name', 'event', 'matcher', 'action', 'text', 'scope', 'project', 'enabled']);
    expect(changesOf(BASE, null).every((change) => change.from === null)).toBe(true);
  });

  it('lists a hook that an agent moved to another folder', () => {
    const moved: Hook = { ...BASE, project: '/work/other-app' };

    expect(changesOf(moved, BASE)).toEqual([{ field: 'project', from: '/work/app', to: '/work/other-app' }]);
  });

  it('lists a hook that an agent made global, with the folder it left', () => {
    const widened: Hook = { ...BASE, scope: 'global', project: null };

    expect(changesOf(widened, BASE)).toEqual([
      { field: 'scope', from: 'project', to: 'global' },
      { field: 'project', from: '/work/app', to: null },
    ]);
  });

  it('is empty when nothing differs', () => {
    expect(changesOf(BASE, { ...BASE })).toEqual([]);
  });
});
