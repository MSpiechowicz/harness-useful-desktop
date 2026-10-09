import { describe, expect, it } from 'vitest';

import { commands, type CommandArgs, type CommandName } from '../commands';
import { IpcFailure } from '../transport';
import { createMockTransport } from './transport';
import { defaultScenario } from './scenarios';
import { samplePrefs } from '../../../../tests/helpers';
import { fixtures } from './fixtures';

/** One valid call per command; typed so adding a command to the map forces an entry here. */
const SAMPLE_ARGS: { [K in CommandName]: CommandArgs<K> } = {
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
};

function transport() {
  return createMockTransport(defaultScenario(), 0);
}

describe('mock transport', () => {
  it.each(Object.keys(commands) as CommandName[])('implements %s', async (name) => {
    await expect(transport().invoke(name, SAMPLE_ARGS[name] as never)).resolves.not.toBeInstanceOf(IpcFailure);
  });

  it('refuses a project that is neither active nor recent', async () => {
    const mock = transport();

    await expect(mock.invoke('config_view', { harness: 'omp', project: '/etc' })).rejects.toMatchObject({ code: 'forbidden' });
    await expect(mock.invoke('extensions_view', { harness: 'omp', project: '/etc' })).rejects.toMatchObject({ code: 'forbidden' });
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
