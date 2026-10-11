import { afterEach, describe, expect, it } from 'vitest';

import { PROJECT, storeWithMock } from '../../../tests/chat-helpers';
import { nav } from './nav.svelte';
import { restoreWorkspace } from './workspace.svelte';

afterEach(() => {
  localStorage.clear();
  nav.go('chat');
});

describe('workspace restore', () => {
  it('reopens the last run’s chats, resuming their sessions, on the page the user left', async () => {
    localStorage.setItem(
      'harness:workspace',
      JSON.stringify({ view: 'changes', chats: [{ harness: 'omp', sessionId: 'abc-123', project: PROJECT }], active: 0 }),
    );
    const { store, of } = storeWithMock();

    expect(await restoreWorkspace(store)).toBe(true);

    expect(of('agent_start')[0]).toMatchObject({ request: { harness: 'omp', project: PROJECT, resume: { harness: 'omp', id: 'abc-123' } } });
    expect(store.sessions).toHaveLength(1);
    expect(store.activeKey).toBe(store.sessions[0].key);
    expect(nav.current).toBe('changes');
  });
});
