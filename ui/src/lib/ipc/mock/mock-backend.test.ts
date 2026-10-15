import { describe, expect, it } from 'vitest';

import { commands, type CommandArgs, type CommandName } from '../commands';
import { IpcFailure } from '../transport';
import { createMockTransport } from './transport';
import { defaultScenario } from './scenarios';
import { samplePrefs } from '../../../../tests/helpers';
import { fixtures } from './fixtures';

/** One valid call per command; typed so adding a command to the map forces an entry here. */
const sampleArgs = (sessionKey: string): { [K in CommandName]: CommandArgs<K> } => ({
  app_info: undefined,
  prefs_get: undefined,
  prefs_set: { prefs: samplePrefs() },
  open_external: { url: 'https://github.com/MSpiechowicz/harness-useful-desktop' },
  reveal_path: { path: '/home/demo/.omp/agent/config.yml' },
  harness_list: undefined,
  harness_detect: { harness: 'omp' },
  harness_pick_binary: { harness: 'pi' },
  harness_clear_binary: { harness: 'omp' },
  config_view: { harness: 'omp', project: null },
  extensions_view: { harness: 'pi', project: null },
  commands_catalog: { harness: 'omp' },
  project_pick: undefined,
  project_open: { path: fixtures.recentProjects[0].path },
  project_close: undefined,
  project_recent: undefined,
  project_forget: { path: fixtures.recentProjects[1].path },
  agent_start: { request: START_REQUEST, onEvent: () => {} },
  agent_send: { key: sessionKey, action: { type: 'getState' } },
  agent_ui_respond: { key: sessionKey, response: { id: 'none', outcome: { type: 'cancelled' } } },
  agent_bash: { key: sessionKey, command: 'ls', confirmed: true },
  agent_move_project: { key: sessionKey, project: fixtures.recentProjects[1].path },
  agent_stop: { key: sessionKey },
  agent_list: undefined,
  sessions_list: { harness: 'omp', project: fixtures.recentProjects[0].path },
  sessions_folder: { harness: 'pi', project: fixtures.recentProjects[0].path },
  session_tree: { session: SESSION, project: fixtures.recentProjects[0].path },
  session_messages: { session: SESSION, project: fixtures.recentProjects[0].path, cursor: null, limit: 50 },
  session_files_touched: { session: SESSION, project: fixtures.recentProjects[0].path },
  session_delete: { session: SESSION, project: fixtures.recentProjects[0].path, emptyCopyOnly: false },
  git_status: { project: fixtures.recentProjects[0].path },
  git_branches: { project: fixtures.recentProjects[0].path },
  git_diff: { project: fixtures.recentProjects[0].path, request: { scope: 'all', paths: [] } },
  git_file_lines: { project: fixtures.recentProjects[0].path, path: 'src/lib.rs', staged: false },
  git_discard_all: { project: fixtures.recentProjects[0].path },
  git_discard_files: { project: fixtures.recentProjects[0].path, paths: ['src/lib.rs'] },
  review_compose: { request: { comments: [{ path: 'src/lib.rs', side: 'new', startLine: 3, endLine: 3, body: 'Why?', snippet: ['mod error;'] }], note: null } },
  ecosystem_status: undefined,
  ecosystem_releases: { installed: [{ id: 'skills', version: '0.2.36' }] },
  dashboard_availability: undefined,
  dashboard_query: { query: { tool: 'limits', args: {} } },
  extension_action: { harness: 'omp', action: { type: 'pluginEnable', name: 'npm-helper' }, dryRun: true, project: null },
  setup_status: undefined,
  setup_install_dashboard: { confirmed: true, onLog: () => {} },
  setup_register_mcp: { harness: 'omp' },
  sync_export_preview: undefined,
  sync_export: { selected: ['omp:marketplace:acme-tools'] },
  sync_export_save: { selected: ['omp:marketplace:acme-tools'] },
  sync_import_preview: { input: { type: 'text', text: 'hud1:abc' } },
  sync_import_apply: { planId: 'plan-1', selected: ['omp:marketplace:harness-useful-skills'], confirmed: true, onProgress: () => {} },
  update_check: undefined,
  update_install: { onProgress: () => {} },
  app_relaunch: undefined,
  launch_sound: { play: false },
  drop_listen: { onDrop: () => {} },
  attachment_save: { name: 'build.log', data: 'aGk=' },
});

const SESSION = { harness: 'omp', id: fixtures.sessions.list.omp[0].ref.id } as const;

const START_REQUEST = { harness: 'omp', project: fixtures.recentProjects[0].path, resume: null, approvalOverride: null, trustOnce: false, utility: false } as const;

function transport() {
  return createMockTransport(defaultScenario(), 0);
}

describe('mock transport', () => {
  it.each(Object.keys(commands) as CommandName[])('implements %s', async (name) => {
    const mock = transport();
    const { key } = await mock.invoke('agent_start', { request: START_REQUEST, onEvent: () => {} });
    await mock.invoke('update_check', undefined);

    // An import applies a plan that a review made first.
    if (name === 'sync_import_apply') {
      await mock.invoke('sync_import_preview', { input: { type: 'file' } });
    }

    await expect(mock.invoke(name, sampleArgs(key)[name] as never)).resolves.not.toBeInstanceOf(IpcFailure);
  });

  it('refuses a project that is neither active nor recent', async () => {
    const mock = transport();

    await expect(mock.invoke('config_view', { harness: 'omp', project: '/etc' })).rejects.toMatchObject({ code: 'forbidden' });
    await expect(mock.invoke('extensions_view', { harness: 'omp', project: '/etc' })).rejects.toMatchObject({ code: 'forbidden' });
  });

  it('refuses session and git requests for a project that is neither active nor recent', async () => {
    const mock = transport();

    await expect(mock.invoke('sessions_list', { harness: 'omp', project: '/etc' })).rejects.toMatchObject({ code: 'forbidden' });
    await expect(mock.invoke('sessions_folder', { harness: 'pi', project: '/etc' })).rejects.toMatchObject({ code: 'forbidden' });
    await expect(mock.invoke('session_tree', { session: SESSION, project: '/etc' })).rejects.toMatchObject({ code: 'forbidden' });
    await expect(mock.invoke('git_status', { project: '/etc' })).rejects.toMatchObject({ code: 'forbidden' });
    await expect(mock.invoke('git_diff', { project: '/etc', request: { scope: 'all', paths: [] } })).rejects.toMatchObject({ code: 'forbidden' });
  });

  it('deletes a session, and only an empty copy when asked to', async () => {
    const mock = transport();
    const project = fixtures.recentProjects[0].path;
    const copy = fixtures.sessions.list.omp.find((summary) => summary.emptyCopy);
    const ids = async () => (await mock.invoke('sessions_list', { harness: 'omp', project })).map((summary) => summary.ref.id);

    await expect(mock.invoke('session_delete', { session: SESSION, project, emptyCopyOnly: true })).rejects.toMatchObject({ code: 'invalidInput' });
    expect(await ids()).toContain(SESSION.id);

    await mock.invoke('session_delete', { session: SESSION, project, emptyCopyOnly: false });
    expect(await ids()).not.toContain(SESSION.id);
    await expect(mock.invoke('session_tree', { session: SESSION, project })).rejects.toMatchObject({ code: 'notFound' });

    await mock.invoke('session_delete', { session: copy!.ref, project, emptyCopyOnly: true });
    expect(await ids()).not.toContain(copy!.ref.id);
  });

  it('sends transcript stats with the first page only', async () => {
    const mock = transport();
    const project = fixtures.recentProjects[0].path;

    const first = await mock.invoke('session_messages', { session: SESSION, project, cursor: null, limit: 2 });
    const next = await mock.invoke('session_messages', { session: SESSION, project, cursor: first.nextCursor, limit: 2 });

    expect(first.stats).toEqual(fixtures.sessions.messages.stats);
    expect(next.stats).toBeNull();
  });

  it('reports a folder that is not a repository as such, with no branches or diffs', async () => {
    const mock = transport();
    const plain = fixtures.recentProjects[1].path;

    await expect(mock.invoke('git_status', { project: plain })).resolves.toMatchObject({ isRepo: false, entries: [] });
    await expect(mock.invoke('git_branches', { project: plain })).resolves.toEqual([]);
    await expect(mock.invoke('git_diff', { project: plain, request: { scope: 'all', paths: [] } })).resolves.toEqual([]);
  });

  it('leaves out the project layer when no project is open', async () => {
    const mock = transport();
    const recent = fixtures.recentProjects[0].path;

    const without = await mock.invoke('config_view', { harness: 'omp', project: null });
    const withProject = await mock.invoke('config_view', { harness: 'omp', project: recent });

    expect(without.sections.some((section) => section.source.layer === 'project')).toBe(false);
    expect(withProject.sections.some((section) => section.source.layer === 'project')).toBe(true);
  });

  it('keeps server-managed preferences out of reach of prefs_set', async () => {
    const mock = transport();
    const prefs = await mock.invoke('prefs_get', undefined);

    const saved = await mock.invoke('prefs_set', {
      prefs: { ...prefs, theme: 'dark', activeProject: '/etc', recentProjects: ['/etc'], binaryOverrides: [{ harness: 'omp', path: '/bin/sh' }] },
    });

    expect(saved.theme).toBe('dark');
    expect(saved.activeProject).toBeNull();
    expect(saved.recentProjects).not.toContain('/etc');
    expect(saved.binaryOverrides).toEqual([]);
  });

  it('clears the recent projects when remembering is turned off', async () => {
    const mock = transport();
    const prefs = await mock.invoke('prefs_get', undefined);

    await mock.invoke('prefs_set', { prefs: { ...prefs, rememberRecentProjects: false } });

    await expect(mock.invoke('project_recent', undefined)).resolves.toEqual([]);
  });
});
