import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ScriptRequestEvent } from '../bindings/ScriptRequestEvent';

import { IpcFailure } from '../transport';
import { memoryScriptsHandlers, scriptScenarioFromLocation, scriptsSupported } from './memory-scripts-handlers';

describe('mock scripts', () => {
  it('lists the global scripts without a project and the repository ones with it, and never sends a body in the list', () => {
    const handlers = memoryScriptsHandlers(() => 'ok');

    const without = handlers.memory_scripts_list({ project: null, offset: 0 });
    const within = handlers.memory_scripts_list({ project: '/work/app', offset: 0 });

    expect(without.scripts.map((script) => script.name)).toEqual(['disk-hogs']);
    expect(within.scripts.map((script) => script.name).sort()).toEqual(['ci-status', 'deploy-check', 'disk-hogs', 'lint-changed', 'open-prs']);
    expect(within.total).toBe(5);
    expect(within.groups).toEqual({ needsReview: 2, repo: 4, global: 1 });
    expect(JSON.stringify(within)).not.toContain('"body"');
    expect(handlers.memory_script_get({ id: 12, project: '/work/app' })).toMatchObject({ name: 'open-prs', body: expect.stringContaining('gh pr list') });
  });

  it('answers a page: the search, the group, the sort and the window, with the counts of every group under the search', () => {
    const handlers = memoryScriptsHandlers(() => 'ok');
    const list = (args: Partial<Parameters<typeof handlers.memory_scripts_list>[0]>) => handlers.memory_scripts_list({ project: '/work/app', offset: 0, ...args });

    expect(list({ sort: 'name' }).scripts.map((script) => script.name)).toEqual(['ci-status', 'deploy-check', 'disk-hogs', 'lint-changed', 'open-prs']);
    expect(list({ sort: 'name', order: 'desc', limit: 2 }).scripts.map((script) => script.name)).toEqual(['open-prs', 'lint-changed']);
    expect(list({ sort: 'name', offset: 4, limit: 2 }).scripts.map((script) => script.name)).toEqual(['open-prs']);

    const review = list({ group: 'needsReview' });
    expect(review.total).toBe(2);
    expect(review.scripts.every((script) => script.approval === 'needed')).toBe(true);

    const searched = list({ query: 'pull requests', group: 'global' });
    expect(searched.total).toBe(0);
    expect(searched.groups).toEqual({ needsReview: 0, repo: 1, global: 0 });
  });

  it('sorts by every column like the server, a script never run last either way, and pages in that order', () => {
    const handlers = memoryScriptsHandlers(() => 'ok');
    const sorted = (sort: Parameters<typeof handlers.memory_scripts_list>[0]['sort'], order: 'asc' | 'desc', offset = 0, limit = 50) =>
      handlers.memory_scripts_list({ project: '/work/app', offset, limit, sort, order }).scripts.map((script) => script.name);

    expect(sorted('runs', 'desc')).toEqual(['lint-changed', 'open-prs', 'deploy-check', 'ci-status', 'disk-hogs']);
    expect(sorted('lastRun', 'asc')).toEqual(['deploy-check', 'ci-status', 'lint-changed', 'open-prs', 'disk-hogs']);
    expect(sorted('lastRun', 'desc')).toEqual(['open-prs', 'lint-changed', 'ci-status', 'deploy-check', 'disk-hogs']);
    expect(sorted('lastRun', 'desc', 3, 2)).toEqual(['deploy-check', 'disk-hogs']);
    expect(sorted('language', 'asc').slice(2)).toEqual(['lint-changed', 'ci-status', 'disk-hogs']);
    expect(sorted('scope', 'asc')[0]).toBe('disk-hogs');
    expect(sorted('place', 'asc')[0]).toBe('ci-status');
    expect(sorted('language', 'desc')).toEqual([...sorted('language', 'asc')].reverse());
  });

  it('deletes a script for good and refuses it afterwards', () => {
    const handlers = memoryScriptsHandlers(() => 'ok');

    handlers.memory_script_delete({ id: 15, project: '/work/app' });

    expect(handlers.memory_scripts_list({ project: '/work/app', offset: 0 }).scripts.map((script) => script.id).sort((a, b) => a - b)).toEqual([12, 21, 27, 30]);
    expect(() => handlers.memory_script_get({ id: 15, project: '/work/app' })).toThrow('script 15 was not found');
  });

  it('deletes every script the project sees at once and counts the ones the team database kept', () => {
    const handlers = memoryScriptsHandlers(() => 'ok');
    const total = (project: string | null) => handlers.memory_scripts_list({ project, offset: 0 }).total;

    expect(handlers.memory_scripts_delete_all({ project: null })).toEqual({ deleted: 1, team: 1 });
    expect(total(null)).toBe(0);
    expect(total('/work/app')).toBe(4);

    expect(handlers.memory_scripts_delete_all({ project: '/work/app' })).toEqual({ deleted: 4, team: 3 });
    expect(total('/work/app')).toBe(0);
    expect(() => handlers.memory_script_get({ id: 12, project: '/work/app' })).toThrow('script 12 was not found');
  });

  it('refuses to delete them all while the server is missing, too old or failing', () => {
    for (const [scenario, code] of [['notInstalled', 'notFound'], ['outdated', 'unsupported'], ['failed', 'process']] as const) {
      const refused = (() => {
        try {
          memoryScriptsHandlers(() => scenario).memory_scripts_delete_all({ project: '/work/app' });
        } catch (error) {
          return error instanceof IpcFailure ? error.code : 'other';
        }

        return 'none';
      })();

      expect(refused).toBe(code);
    }
  });

  it('reports a missing, old or failing server through the availability, with no scripts', () => {
    for (const [scenario, kind] of [['notInstalled', 'notInstalled'], ['outdated', 'outdated'], ['failed', 'failed']] as const) {
      const page = memoryScriptsHandlers(() => scenario).memory_scripts_list({ project: '/work/app', offset: 0 });

      expect(page.availability.kind).toBe(kind);
      expect(page.scripts).toEqual([]);
    }

    expect(scriptsSupported('outdated')).toBe(false);
    expect(scriptsSupported('failed')).toBe(true);
    expect(scriptScenarioFromLocation('?scripts=empty')).toBe('empty');
    expect(scriptScenarioFromLocation('?scripts=nope')).toBe('ok');
  });

  it('marks the scripts a teammate wrote or changed as needing review, and approves them when trusted', () => {
    const handlers = memoryScriptsHandlers(() => 'ok');
    const approvals = () => Object.fromEntries(handlers.memory_scripts_list({ project: '/work/app', offset: 0 }).scripts.map((script) => [script.name, script.approval]));

    expect(approvals()).toEqual({ 'open-prs': 'approved', 'ci-status': 'local', 'disk-hogs': 'needed', 'deploy-check': 'needed', 'lint-changed': 'team' });

    const reviewed = (id: number) => ({ id, sha256: handlers.memory_script_get({ id, project: '/work/app' }).sha256 });
    handlers.memory_scripts_trust({ items: [reviewed(21), reviewed(27)], project: '/work/app' });

    expect(approvals()).toMatchObject({ 'disk-hogs': 'approved', 'deploy-check': 'approved' });
    expect(handlers.memory_script_get({ id: 21, project: '/work/app' })).toMatchObject({ trusted: true });
    expect(() => handlers.memory_scripts_trust({ items: [{ id: 999, sha256: 'a'.repeat(64) }], project: '/work/app' })).toThrow('script 999 was not found');
  });

  it('refuses a script whose version is not the one reviewed, and approves the others', () => {
    const handlers = memoryScriptsHandlers(() => 'ok');
    const reviewed = (id: number) => ({ id, sha256: handlers.memory_script_get({ id, project: '/work/app' }).sha256 });

    expect(() => handlers.memory_scripts_trust({ items: [reviewed(21), { id: 27, sha256: 'f'.repeat(64) }], project: '/work/app' })).toThrow('changed since it was reviewed');

    expect(handlers.memory_script_get({ id: 21, project: '/work/app' })).toMatchObject({ approval: 'approved' });
    expect(handlers.memory_script_get({ id: 27, project: '/work/app' })).toMatchObject({ approval: 'needed' });
  });
});

describe('mock script run requests', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  /** Starts the listener of a fresh mock and collects what it is sent. */
  function listening(scenario: 'ok' | 'request') {
    vi.useFakeTimers();
    const handlers = memoryScriptsHandlers(() => scenario);
    const events: ScriptRequestEvent[] = [];

    handlers.memory_script_requests_listen({ onRequest: (event) => events.push(event) });

    return { handlers, events };
  }

  it('sends two requests in the request scenario, a changed script with its approved version and a new one without', () => {
    const { events } = listening('request');
    vi.advanceTimersByTime(1);

    const requests = events.flatMap((event) => (event.type === 'request' ? [event.request] : []));

    expect(requests.map((request) => request.script.name)).toEqual(['deploy-check', 'disk-hogs']);
    expect(requests[0].previous?.body).toContain('staging.example.com/health');
    expect(requests[0].previous?.body).not.toBe(requests[0].body);
    expect(requests[1].previous).toBeNull();
    expect(requests.every((request) => /^[0-9a-f]{32}$/.test(request.requestId))).toBe(true);
  });

  it('sends nothing in the other scenarios', () => {
    const { events } = listening('ok');
    vi.advanceTimersByTime(60_000);

    expect(events).toEqual([]);
  });

  it('approves the script when the user allows it, and refuses an answer to a request that is over', () => {
    const { handlers, events } = listening('request');
    vi.advanceTimersByTime(1);
    const [first] = events.flatMap((event) => (event.type === 'request' ? [event.request] : []));

    handlers.memory_script_decide({ requestId: first.requestId, allow: true });

    expect(handlers.memory_script_get({ id: first.script.id, project: '/work/app' })).toMatchObject({ approval: 'approved', trusted: true });
    expect(() => handlers.memory_script_decide({ requestId: first.requestId, allow: true })).toThrow('stopped waiting');
  });

  it('leaves the script as it was when the user declines, and reports a request gone when nobody answers in the server window', () => {
    const { handlers, events } = listening('request');
    vi.advanceTimersByTime(1);
    const [first, second] = events.flatMap((event) => (event.type === 'request' ? [event.request] : []));

    handlers.memory_script_decide({ requestId: first.requestId, allow: false });
    expect(handlers.memory_script_get({ id: first.script.id, project: '/work/app' })).toMatchObject({ approval: 'needed' });

    vi.advanceTimersByTime(45_000);

    expect(events.filter((event) => event.type === 'gone')).toEqual([{ type: 'gone', requestId: second.requestId }]);
  });
});
