import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { nav } from '$lib/stores/nav.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { showNote, showScript } from './memory-open';

const scripts = (...named: [number, string][]) => ({ availability: { kind: 'available' }, scripts: named.map(([id, name]) => ({ id, name })) });

beforeEach(() => {
  nav.current = 'chat';
  viewState.reset('notes');
  viewState.reset('scripts');
  viewState.memoryReach = 'all';
});

afterEach(() => vi.restoreAllMocks());

describe('showing a note from the chat', () => {
  it('opens the note in the Notes view', () => {
    showNote(12);

    expect(viewState.noteId).toBe(12);
    expect(nav.current).toBe('notes');
  });

  it('widens a single repository to every repository and keeps the other filters', () => {
    viewState.memoryReach = 'repo:git:github.com/acme/widget';
    viewState.notesKind = 'fix';
    viewState.notesQuery = 'webkit';

    showNote(12);

    expect(viewState.memoryReach).toBe('all');
    expect(viewState.notesKind).toBe('fix');
    expect(viewState.notesQuery).toBe('webkit');
  });

  it('widens the global-only reach for a repository note, not for a global one', () => {
    viewState.memoryReach = 'global';
    showNote(1, 'global');
    expect(viewState.memoryReach).toBe('global');

    showNote(2, 'repo');
    expect(viewState.memoryReach).toBe('all');
  });

  it('leaves the reach of every repository alone', () => {
    showNote(3, 'repo');

    expect(viewState.memoryReach).toBe('all');
  });
});

describe('showing a script from the chat', () => {
  it('opens a script it knows by id without asking for the list', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    viewState.memoryReach = 'repo:git:github.com/acme/widget';

    await showScript(31);

    expect(invoke).not.toHaveBeenCalled();
    expect(viewState.scriptId).toBe(31);
    expect(viewState.memoryReach).toBe('all');
    expect(nav.current).toBe('scripts');
  });

  it('finds a script by the exact name among every repository and opens it', async () => {
    const invoke = vi.spyOn(ipc, 'invoke').mockResolvedValue(scripts([4, 'snapshot-all'], [8, 'snapshot']) as never);

    await showScript('snapshot');

    expect(invoke).toHaveBeenCalledWith('memory_scripts_list', expect.objectContaining({ project: null, repos: { kind: 'all' }, offset: 0, query: 'snapshot' }));
    expect(viewState.scriptId).toBe(8);
    expect(nav.current).toBe('scripts');
  });

  it('prefers the script of the chat project over a global one and another repository of the same name', async () => {
    const listed = {
      availability: { kind: 'available' },
      scripts: [
        { id: 1, name: 'snapshot', scope: 'repo', repo: 'github.com/acme/other' },
        { id: 2, name: 'snapshot', scope: 'global', repo: null },
        { id: 3, name: 'snapshot', scope: 'repo', repo: 'github.com/acme/widget' },
      ],
    };
    const invoke = vi.spyOn(ipc, 'invoke').mockImplementation((async (command: string) => (command === 'project_scopes' ? [{ path: '/work/widget', name: 'widget', root: '/work/widget', key: 'git:github.com/acme/widget' }] : listed)) as never);

    await showScript('snapshot', '/work/widget');
    expect(viewState.scriptId).toBe(3);

    // Another project, or one without a key: the global script, then the first.
    viewState.reset('scripts');
    await showScript('snapshot', '/work/elsewhere');
    expect(viewState.scriptId).toBe(2);

    viewState.reset('scripts');
    invoke.mockImplementation((async () => ({ ...listed, scripts: [listed.scripts[0]] })) as never);
    await showScript('snapshot', '/work/widget');
    expect(viewState.scriptId).toBe(1);
  });

  it('leaves the Scripts view searching for a name it cannot find or read', async () => {
    vi.spyOn(ipc, 'invoke').mockResolvedValueOnce(scripts([4, 'snapshot-all']) as never);
    await showScript('snapshot');
    expect(viewState.scriptId).toBeNull();
    expect(viewState.scriptsQuery).toBe('snapshot');

    viewState.scriptsQuery = '';
    nav.current = 'chat';
    vi.spyOn(ipc, 'invoke').mockRejectedValueOnce(new Error('offline'));
    await showScript('other');
    expect(viewState.scriptsQuery).toBe('other');
    expect(nav.current).toBe('scripts');
  });
});
