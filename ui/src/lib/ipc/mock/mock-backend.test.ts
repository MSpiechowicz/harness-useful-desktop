import { afterEach, describe, expect, it } from 'vitest';

import { commands, type CommandArgs, type CommandName } from '../commands';
import { IpcFailure } from '../transport';
import { createMockTransport } from './transport';
import { defaultScenario } from './scenarios';
import { manualRemovalsOf } from './setup-handlers';
import { samplePrefs } from '../../../../tests/helpers';
import { fixtures } from './fixtures';
import { sampleScripts } from './memory-scripts-fixtures';
import type { ScriptLanguage } from '../bindings/ScriptLanguage';
import { MEMORY_EMBEDDER_ARGS } from './mock-backend-embedder-args';
import { MEMORY_NOTE_ARGS } from './mock-backend-note-args';
import { memoryGuardArgs } from './mock-backend-guard-args';

/** One valid call per command; typed so adding a command to the map forces an entry here. */
const sampleArgs = (sessionKey: string): { [K in CommandName]: CommandArgs<K> } => ({
  app_info: undefined,
  prefs_get: undefined,
  prefs_set: { prefs: samplePrefs() },
  memory_set_enabled: { enabled: false },
  ...MEMORY_EMBEDDER_ARGS,
  memory_remote_get: undefined,
  memory_remote_set: { enabled: true, kind: 'postgres', url: 'postgres://memory:pw@db.ok.example.com/harness_memory', token: null, repositoryNotes: 'remote', globalNotes: 'local', scripts: 'remote', trustTeamScripts: false },
  memory_remote_test: { kind: 'libsql', url: 'libsql://team-ok.turso.io', token: 'tok' },
  memory_repos_list: { kind: 'scripts' },
  memory_scripts_list: { project: '/home/demo/project', offset: 0 },
  memory_script_get: { id: 12, project: '/home/demo/project' },
  memory_script_delete: { id: 12, project: '/home/demo/project' },
  memory_script_hide: { id: 12, project: '/home/demo/project', hidden: true },
  memory_scripts_discard_local: { ids: 'all' },
  memory_script_save: {
    project: '/home/demo/project',
    draft: { name: 'open-prs', description: 'Edited in the app', language: 'bash', body: 'echo app\n', timeoutSecs: 60, scope: 'repo' },
    replace: true,
    base: sampleScripts().find((script) => script.id === 12)?.sha256 ?? '',
  },
  memory_scripts_delete_all: { project: '/home/demo/project' },
  ...MEMORY_NOTE_ARGS,
  ...memoryGuardArgs(sessionKey),
  memory_scripts_trust: { items: [{ id: 21, sha256: sampleScripts().find((script) => script.id === 21)?.sha256 ?? '' }], project: '/home/demo/project' },
  memory_script_requests_listen: { onRequest: () => {} },
  memory_script_decide: { requestId: 'a'.repeat(32), allow: false },
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
  project_scopes: undefined,
  agent_start: { request: START_REQUEST, onEvent: () => {} },
  agent_send: { key: sessionKey, action: { type: 'getState' } },
  agent_ui_respond: { key: sessionKey, response: { id: 'none', outcome: { type: 'cancelled' } } },
  agent_bash: { key: sessionKey, command: 'ls', confirmed: true },
  agent_move_project: { key: sessionKey, project: fixtures.recentProjects[1].path },
  agent_stop: { key: sessionKey },
  agent_list: undefined,
  sessions_list: { harness: 'omp', project: fixtures.recentProjects[0].path },
  harness_session_totals: { harness: 'omp' },
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
  setup_install_memory: { confirmed: true, onLog: () => {} },
  setup_install_opencode_skills: { confirmed: true, onLog: () => {} },
  setup_install_codex_skills: { confirmed: true, onLog: () => {} },
  skills_migrate_names: undefined,
  setup_register_mcp: { harness: 'omp' },
  setup_register_memory: { harness: 'omp' },
  setup_register_desktop_mcp: { harness: 'omp' },
  sync_export_preview: undefined,
  sync_export: { selected: ['omp:marketplace:acme-tools'] },
  sync_export_save: { selected: ['omp:marketplace:acme-tools'] },
  sync_import_preview: { input: { type: 'text', text: 'hud1:abc' } },
  sync_import_apply: { planId: 'plan-1', selected: ['omp:marketplace:harness-useful-skills'], confirmed: true, onProgress: () => {} },
  hooks_list: { project: fixtures.recentProjects[0].path },
  hook_save: { draft: { id: null, expectedSha256: null, name: 'Check types', description: null, scope: 'global', project: null, event: 'turnEnd', matcher: null, action: { type: 'followUp', text: 'Run the type checker.' }, enabled: true } },
  hook_delete: { id: '0'.repeat(32), sha256: '' },
  hook_approve: { id: '0'.repeat(32), sha256: '' },
  hook_reject: { id: '0'.repeat(32), sha256: '' },
  hooks_family: undefined,
  hooks_family_apply: { seeds: ['dashboard-limits'] },
  hooks_delete_all: {},
  hooks_listen: { onEvent: () => {} },
  hooks_share: { id: '0'.repeat(32), sha256: '' },
  hooks_team_remove: { id: '0'.repeat(32), sha256: '' },
  hooks_team_refresh: { project: fixtures.recentProjects[0].path },
  hooks_team_copy: { id: '0'.repeat(32), sha256: '', project: fixtures.recentProjects[0].path },
  hooks_team_enable: { id: '0'.repeat(32), enabled: false },
  routines_list: { project: fixtures.recentProjects[0].path },
  routine_get: { id: '0'.repeat(32) },
  routine_save: { draft: { id: null, expectedSha256: null, name: 'Look at the queue', description: null, scope: 'global', project: null, when: { type: 'daily', at: '09:00' }, action: { type: 'command', command: 'git status', timeoutSecs: null }, enabled: true } },
  routine_delete: { id: '0'.repeat(32), sha256: '' },
  routines_delete_all: {},
  routine_approve: { id: '0'.repeat(32), sha256: '' },
  routine_reject: { id: '0'.repeat(32), sha256: '' },
  routines_listen: { onEvent: () => {} },
  routine_run_now: { id: '0'.repeat(32) },
  routine_run_stop: { runId: '0'.repeat(32) },
  routine_runs: { query: { routineId: null, status: null, offset: 0, limit: 50 } },
  routine_schedule_preview: { when: { type: 'weekdays', at: '09:00' } },
  routine_run_attach: { runId: '0'.repeat(32), onEvent: () => {} },
  routines_workspace: undefined,
  routines_share: { id: '0'.repeat(32), sha256: '' },
  routines_team_remove: { id: '0'.repeat(32), sha256: '' },
  routines_team_remove_mine: undefined,
  routines_team_refresh: { project: fixtures.recentProjects[0].path },
  routines_team_copy: { id: '0'.repeat(32), sha256: '', project: fixtures.recentProjects[0].path },
  routines_team_enable: { id: '0'.repeat(32), enabled: false },
  update_check: undefined,
  update_install: { onProgress: () => {} },
  app_relaunch: undefined,
  launch_sound: { play: false },
  drop_listen: { onDrop: () => {} },
  attachment_save: { name: 'build.log', data: 'aGk=' },
});

const SESSION = { harness: 'omp', id: fixtures.sessions.list.omp[0].ref.id } as const;
const START_REQUEST = { harness: 'omp', project: fixtures.recentProjects[0].path, resume: null, approvalOverride: null, trustOnce: false, utility: false, memoryChat: null } as const;

const transport = () => createMockTransport(defaultScenario(), 0);

/** The commands that need a team to work on: the `?hooks=team` preview has hooks to share and team hooks to remove, copy and switch. */
const TEAM_COMMANDS = ['hooks_share', 'hooks_team_remove', 'hooks_team_refresh', 'hooks_team_copy', 'hooks_team_enable'] as const;

async function teamCommandResolves(name: (typeof TEAM_COMMANDS)[number]): Promise<void> {
  globalThis.history.replaceState(null, '', '/?hooks=team');

  try {
    const mock = transport();
    const project = fixtures.recentProjects[0].path;
    const { hooks } = await mock.invoke('hooks_list', { project });
    const local = hooks.find((hook) => hook.origin === 'local' && hook.shareBlock === null);
    const team = hooks.find((hook) => hook.origin === 'team' && hook.status !== 'duplicate');
    const args = {
      hooks_share: { id: local?.id ?? '', sha256: local?.sha256 ?? '' },
      hooks_team_remove: { id: team?.id ?? '', sha256: team?.sha256 ?? '' },
      hooks_team_refresh: { project },
      hooks_team_copy: { id: team?.id ?? '', sha256: team?.sha256 ?? '', project },
      hooks_team_enable: { id: team?.id ?? '', enabled: false },
    }[name];

    await expect(mock.invoke(name, args as never)).resolves.not.toBeInstanceOf(IpcFailure);
  } finally {
    globalThis.history.replaceState(null, '', '/');
  }
}

type RoutineChange = 'routine_get' | 'routine_delete' | 'routine_approve' | 'routine_reject' | 'routine_run_now' | 'routine_run_stop' | 'routine_run_attach';

/** Runs one of the commands that act on a sample routine or run on the routine or run it needs. */
async function routineCommandResolves(mock: ReturnType<typeof transport>, name: RoutineChange): Promise<void> {
  const { routines } = await mock.invoke('routines_list', { project: fixtures.recentProjects[0].path });
  const pending = routines.find((routine) => routine.status === 'pending');
  const idle = routines.find((routine) => routine.status === 'active');
  const { runs } = await mock.invoke('routine_runs', { query: { routineId: null, status: 'running', offset: 0, limit: 5 } });
  const args = {
    routine_get: { id: idle?.id ?? '' },
    routine_delete: { id: pending?.id ?? '', sha256: pending?.sha256 ?? '' },
    routine_approve: { id: pending?.id ?? '', sha256: pending?.sha256 ?? '' },
    routine_reject: { id: pending?.id ?? '', sha256: pending?.sha256 ?? '' },
    routine_run_now: { id: idle?.id ?? '' },
    routine_run_stop: { runId: runs[0]?.id ?? '' },
    routine_run_attach: { runId: runs.find((run) => run.session)?.id ?? '', onEvent: () => {} },
  }[name];

  await expect(mock.invoke(name, args as never)).resolves.not.toBeInstanceOf(IpcFailure);
}

/** The commands that need a team to work on: the `?routines=team` preview has routines to share and team routines to remove, copy and switch. */
const TEAM_ROUTINE_COMMANDS = ['routines_share', 'routines_team_remove', 'routines_team_remove_mine', 'routines_team_refresh', 'routines_team_copy', 'routines_team_enable'] as const;

async function teamRoutineCommandResolves(name: (typeof TEAM_ROUTINE_COMMANDS)[number]): Promise<void> {
  globalThis.history.replaceState(null, '', '/?routines=team');

  try {
    const mock = transport();
    const project = fixtures.recentProjects[0].path;
    const { routines } = await mock.invoke('routines_list', { project });
    const local = routines.find((routine) => routine.origin === 'local' && routine.shareBlock === null);
    const team = routines.find((routine) => routine.origin === 'team' && routine.status !== 'duplicate');
    const args = {
      routines_share: { id: local?.id ?? '', sha256: local?.sha256 ?? '' },
      routines_team_remove: { id: team?.id ?? '', sha256: team?.sha256 ?? '' },
      routines_team_remove_mine: undefined,
      routines_team_refresh: { project },
      routines_team_copy: { id: team?.id ?? '', sha256: team?.sha256 ?? '', project },
      routines_team_enable: { id: team?.id ?? '', enabled: true, project },
    }[name];

    await expect(mock.invoke(name, args as never)).resolves.not.toBeInstanceOf(IpcFailure);
  } finally {
    globalThis.history.replaceState(null, '', '/');
  }
}

describe('mock transport', () => {
  it.each(Object.keys(commands) as CommandName[])('implements %s', async (name) => {
    if ((TEAM_COMMANDS as readonly string[]).includes(name)) {
      await teamCommandResolves(name as (typeof TEAM_COMMANDS)[number]);

      return;
    }

    if ((TEAM_ROUTINE_COMMANDS as readonly string[]).includes(name)) {
      await teamRoutineCommandResolves(name as (typeof TEAM_ROUTINE_COMMANDS)[number]);

      return;
    }

    const mock = transport();
    const { key } = await mock.invoke('agent_start', { request: START_REQUEST, onEvent: () => {} });
    await mock.invoke('update_check', undefined);

    // An import applies a plan that a review made first.
    if (name === 'sync_import_apply') {
      await mock.invoke('sync_import_preview', { input: { type: 'file' } });
    }

    if (name === 'memory_notes_import_apply') {
      await mock.invoke('memory_notes_import_preview', { project: '/home/demo/project' });
    }

    // A page of a copy review, and the copy itself, belong to a comparison made first.
    if (name === 'memory_notes_copy_rows' || name === 'memory_notes_copy_apply') {
      await mock.invoke('memory_notes_copy_preview', { kind: 'notes' });
    }

    // The memory server is registered only once it is installed.
    if (name === 'setup_register_memory') {
      await mock.invoke('setup_install_memory', { confirmed: true, onLog: () => {} });
    }

    // A change to a hook names the version it was read at: the pending sample hook is read first.
    if (name === 'hook_delete' || name === 'hook_approve' || name === 'hook_reject') {
      const { hooks } = await mock.invoke('hooks_list', { project: fixtures.recentProjects[0].path });
      const waiting = hooks.find((hook) => hook.status === 'pending');

      await expect(mock.invoke(name, { id: waiting?.id ?? '', sha256: waiting?.sha256 ?? '' })).resolves.not.toBeInstanceOf(IpcFailure);

      return;
    }

    // A change to a routine names the version it was read at: the sample routines are read first.
    if (name === 'routine_get' || name === 'routine_delete' || name === 'routine_approve' || name === 'routine_reject' || name === 'routine_run_now' || name === 'routine_run_stop' || name === 'routine_run_attach') {
      await routineCommandResolves(mock, name);

      return;
    }

    // An answer to a request nobody asked is refused, as the backend refuses one for a request that is over.
    if (name === 'memory_script_decide') {
      await expect(mock.invoke(name, sampleArgs(key)[name])).rejects.toMatchObject({ code: 'notFound' });

      return;
    }

    await expect(mock.invoke(name, sampleArgs(key)[name] as never)).resolves.not.toBeInstanceOf(IpcFailure);
  });

  describe('the outdated Memory preview', () => {
    afterEach(() => globalThis.history.replaceState(null, '', '/'));

    it('lists scripts once Harness Memory was updated', async () => {
      globalThis.history.replaceState(null, '', '/?scripts=outdated');
      const mock = transport();

      expect((await mock.invoke('memory_scripts_list', { project: null, offset: 0 })).availability).toEqual({ kind: 'outdated' });

      await mock.invoke('setup_install_memory', { confirmed: true, onLog: () => {} });

      expect((await mock.invoke('memory_scripts_list', { project: null, offset: 0 })).availability).toEqual({ kind: 'available' });
    });

    /** The Memory release the Ecosystem page would compare with what is installed. */
    async function memoryRelease(mock: ReturnType<typeof transport>) {
      const status = await mock.invoke('ecosystem_status', undefined);
      const memory = status.items.find((item) => item.id === 'memory');
      const releases = await mock.invoke('ecosystem_releases', { installed: [{ id: 'memory', version: memory?.version ?? '1.0.0' }] });

      return { version: memory?.version, release: releases.find((release) => release.id === 'memory') };
    }

    it('has a newer Harness Memory release to update to, until it was updated', async () => {
      globalThis.history.replaceState(null, '', '/?scripts=outdated');
      const mock = transport();

      expect((await memoryRelease(mock)).release).toMatchObject({ latestVersion: '1.1.0', updateAvailable: true });

      await mock.invoke('setup_install_memory', { confirmed: true, onLog: () => {} });

      expect((await memoryRelease(mock)).release).toMatchObject({ updateAvailable: false });
    });

    it('is a source build with no newer release in the outdatedNoRelease preview', async () => {
      globalThis.history.replaceState(null, '', '/?scripts=outdatedNoRelease');
      const mock = transport();

      expect((await mock.invoke('memory_scripts_list', { project: null, offset: 0 })).availability).toEqual({ kind: 'outdated' });
      expect(await memoryRelease(mock)).toMatchObject({ version: '1.0.1-dev', release: { updateAvailable: false } });
    });
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

  it('reads an OpenCode transcript as plain user, assistant and tool messages, with no branch summary', async () => {
    const mock = transport();
    const project = fixtures.recentProjects[0].path;
    const session = { harness: 'opencode', id: 'ses_5f3a9c2d8e1bAq7Zr4Kd0Wn6Xm' } as const;

    const page = await mock.invoke('session_messages', { session, project, cursor: null, limit: 50 });

    expect(new Set(page.messages.map((message) => message.role))).toEqual(new Set(['user', 'assistant', 'toolResult']));
    expect(page.total).toBe(page.messages.length);
    expect(page.stats?.messages).toBe(page.messages.length);
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
      prefs: { ...prefs, theme: 'dark', activeProject: '/etc', recentProjects: ['/etc'], binaryOverrides: [{ harness: 'omp', path: '/bin/sh' }], memoryEnabled: false },
    });

    expect(saved.theme).toBe('dark');
    expect(saved.memoryEnabled).toBe(true);
    expect(saved.activeProject).toBeNull();
    expect(saved.recentProjects).not.toContain('/etc');
    expect(saved.binaryOverrides).toEqual([]);
  });

  it('turns project memory off and on through its own command', async () => {
    const mock = transport();

    await expect(mock.invoke('memory_set_enabled', { enabled: false })).resolves.toMatchObject({ memoryEnabled: false });
    await expect(mock.invoke('prefs_get', undefined)).resolves.toMatchObject({ memoryEnabled: false });
    await expect(mock.invoke('memory_set_enabled', { enabled: true })).resolves.toMatchObject({ memoryEnabled: true });
  });

  it('turns memory off with nothing to remove by hand, except in the commented OpenCode config scenarios', async () => {
    const mock = transport();

    await expect(mock.invoke('memory_set_enabled', { enabled: false })).resolves.toMatchObject({ manualRemovals: [] });
    expect(manualRemovalsOf('manualEdit')).toEqual([{ harness: 'opencode', file: '~/.config/opencode/opencode.jsonc', name: 'harness-memory' }]);
    expect(manualRemovalsOf('configured')).toEqual([]);
  });

  it('saves, tests and removes the team database without returning the URL or token', async () => {
    const mock = transport();
    await expect(mock.invoke('memory_remote_get', undefined)).resolves.toMatchObject({ enabled: false, kind: null, problem: null });

    const saved = await mock.invoke('memory_remote_set', { enabled: true, kind: 'libsql', url: 'libsql://team-ok.turso.io', token: 'tok-secret', repositoryNotes: 'local', globalNotes: 'remote', scripts: 'local', trustTeamScripts: false });
    expect(saved).toMatchObject({ enabled: true, kind: 'libsql', urlRedacted: 'libsql://team-ok.turso.io', hasToken: true, repositoryNotes: 'local', scripts: 'local', scriptsSupported: true });
    expect(JSON.stringify(saved)).not.toContain('tok-secret');

    await expect(mock.invoke('memory_remote_test', { kind: 'libsql', url: null, token: null })).resolves.toMatchObject({ ok: true });
    await expect(mock.invoke('memory_remote_test', { kind: 'postgres', url: 'postgres://u:pw@db.example.com/x', token: null })).resolves.toEqual({
      ok: false,
      message: 'error: cannot connect to postgres://u@db.example.com/x: connection refused',
    });

    const pw = { enabled: true, kind: 'postgres', url: null, token: null, repositoryNotes: 'remote', globalNotes: 'remote', scripts: 'remote', trustTeamScripts: false } as const;
    await expect(mock.invoke('memory_remote_set', pw)).rejects.toMatchObject({ code: 'invalidInput' });
    await expect(mock.invoke('memory_remote_set', { ...pw, kind: 'libsql', url: 'libsql://u:pw@t.turso.io' })).rejects.toMatchObject({ code: 'invalidInput' });

    await expect(mock.invoke('memory_remote_set', { ...pw, enabled: false, kind: null })).resolves.toMatchObject({ enabled: false, hasToken: false });
  });

  it('keeps a waiting script waiting when an edit leaves its code as the agent wrote it', async () => {
    const mock = transport();
    const project = '/home/demo/project';
    const waiting = await mock.invoke('memory_script_get', { id: 21, project });
    const draft = { name: waiting.name, description: 'Edited in the app', language: 'bash', body: waiting.body, timeoutSecs: 90, scope: waiting.scope } as const;

    const kept = await mock.invoke('memory_script_save', { project, draft, replace: true, base: waiting.sha256 });
    expect(kept.script).toMatchObject({ id: 21, approval: 'needed', trusted: false });

    const crlf = { ...draft, body: `${waiting.body.replaceAll('\n', '  \r\n')}\n` };
    const endings = await mock.invoke('memory_script_save', { project, draft: crlf, replace: true, base: kept.script?.sha256 });
    expect(endings.script).toMatchObject({ id: 21, approval: 'needed', trusted: false });

    await expect(mock.invoke('memory_script_save', { project, draft, replace: true, base: waiting.sha256 })).rejects.toMatchObject({ code: 'conflict' });

    const changed = await mock.invoke('memory_script_save', { project, draft: { ...draft, body: `${waiting.body}echo done\n` }, replace: true, base: endings.script?.sha256 });
    expect(changed.script).toMatchObject({ id: 21, approval: 'approved', trusted: true });
  });

  it('refuses an edit of a script deleted since the editor opened it', async () => {
    const mock = transport();
    const project = '/home/demo/project';
    const opened = await mock.invoke('memory_script_get', { id: 21, project });
    const draft = { name: opened.name, description: opened.description, language: opened.language as ScriptLanguage, body: `${opened.body}echo done\n`, timeoutSecs: opened.timeoutSecs, scope: opened.scope };

    await mock.invoke('memory_script_delete', { id: 21, project });

    await expect(mock.invoke('memory_script_save', { project, draft, replace: true, base: opened.sha256 })).rejects.toMatchObject({ code: 'conflict' });
    await expect(mock.invoke('memory_script_get', { id: 21, project })).rejects.toMatchObject({ code: 'notFound' });
  });

  it('applies one notes import at a time', async () => {
    const mock = transport();
    const project = '/home/demo/project';
    const preview = await mock.invoke('memory_notes_import_preview', { project });
    const planId = preview?.kind === 'plan' ? preview.plan.planId : '';
    const args = { planId, rows: [0], defaultScope: 'repo' as const, replace: false, project, onProgress: () => {} };

    const running = mock.invoke('memory_notes_import_apply', args);
    await expect(mock.invoke('memory_notes_import_apply', args)).rejects.toMatchObject({ code: 'invalidInput', message: 'an import is already running' });
    await expect(running).resolves.toMatchObject({ created: 1 });
  });
});
