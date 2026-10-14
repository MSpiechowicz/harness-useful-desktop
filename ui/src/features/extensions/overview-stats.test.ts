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
    expect(item.tab === 'marketplaces' && item.updates).toEqual({ tool: '0.3.0' });
  });
});
