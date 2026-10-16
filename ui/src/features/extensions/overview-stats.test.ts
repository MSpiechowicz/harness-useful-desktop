import { describe, expect, it } from 'vitest';

import type { ExtItem } from './extension-items';
import { overviewStats } from './extension-detail';

const entry = (tab: string, data: object) => ({ tab, data }) as unknown as ExtItem;

describe('overviewStats', () => {
  it('counts a plugin\'s features and says whether an update is waiting', () => {
    const features = [{ name: 'a', enabled: true }, { name: 'b', enabled: false }, { name: 'c', enabled: true }];

    expect(overviewStats(entry('plugins', { features, updateAvailable: true, latestVersion: '1.3.0' })).map((stat) => stat.value)).toEqual(['3', '2', '1', 'v1.3.0']);
    expect(overviewStats(entry('plugins', { features, updateAvailable: false, latestVersion: null }))[3].value).toBe('Up to date');
  });

  it('shows no counts for a plugin that lists no features and has no update', () => {
    expect(overviewStats(entry('plugins', { features: [], updateAvailable: false, latestVersion: null }))).toEqual([]);
  });

  it('counts what a marketplace lists and how much of it is installed', () => {
    const entries = [{ installed: true }, { installed: false }, { installed: false }];

    expect(overviewStats(entry('marketplaces', { entries, error: null })).map((stat) => stat.value)).toEqual(['3', '1', '2', 'Readable']);
    expect(overviewStats(entry('marketplaces', { entries: [], error: 'x' }))[3].value).toBe('Unreadable');
  });

  it('counts an MCP server\'s hidden values wherever they sit', () => {
    const server = { args: [{ kind: 'text', value: 'x' }, { kind: 'secret', value: {} }], url: { kind: 'secret', value: {} }, details: [] };
    const [args, settings, hidden, depth] = overviewStats(entry('mcp', server));

    expect([args.value, settings.value, hidden.value, depth.value]).toEqual(['2', '0', '2', '—']);
  });

  it('has nothing to count for a skill', () => {
    expect(overviewStats(entry('skills', {}))).toEqual([]);
  });
});

describe('a marketplace whose plugin is outdated', () => {
  it('says so in its row and its catalog, though its cached catalog still lists the old version', async () => {
    const { buildItems, rowInfo } = await import('./extension-items');
    const view = {
      plugins: [{ id: 'tool@market', name: 'tool', marketplace: null, updateAvailable: true, latestVersion: '0.3.0', features: [], version: '0.2.41' }],
      marketplaces: [{ name: 'market', source: 'a/b', sourceType: 'github', error: null, entries: [{ name: 'tool', version: '0.2.41', installed: true }] }],
    } as never;

    const [item] = buildItems('marketplaces', view);

    expect(rowInfo(item).update).toBe(true);
    expect(item.tab === 'marketplaces' && Object.keys(item.updates)).toEqual(['tool']);
  });
});

describe('a package row', () => {
  it('has the status dot on when the package is installed and off when settings only list it', async () => {
    const { buildItems, rowInfo } = await import('./extension-items');
    const pkg = (name: string, installed: boolean) => ({ name, installed, source: { kind: 'text', value: `git:github.com/demo/${name}` }, kind: 'git', scope: 'user', resources: [] });
    const view = { packages: [pkg('here', true), pkg('gone', false)] } as never;

    const [here, gone] = buildItems('packages', view);

    expect(here.title).toBe('here');
    expect(rowInfo(here).status).toBe('on');
    expect(rowInfo(gone).status).toBe('off');
  });
});

describe('the package of a skill', () => {
  it('is found by name and scope, and left unknown when two packages share the name', async () => {
    const { packageOf } = await import('./extension-items');
    const pkg = (owner: string, scope: string) => ({ name: 'tools', installed: true, source: { kind: 'text', value: `git:github.com/${owner}/tools` }, kind: 'git', scope, resources: ['skills'] });
    const skill = { name: 'lint', description: '', scope: 'user', origin: 'package tools', path: '/x/SKILL.md', package: 'tools' } as never;

    const one = packageOf({ packages: [pkg('acme', 'user'), pkg('acme', 'project')] } as never, skill);
    expect(one?.tab === 'packages' && one.data.scope).toBe('user');

    expect(packageOf({ packages: [pkg('acme', 'user'), pkg('evil', 'user')] } as never, skill)).toBeUndefined();
  });
});
