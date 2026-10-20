import { describe, expect, it } from 'vitest';

import type { SyncMcpPreview } from '$lib/ipc/bindings/SyncMcpPreview';

import { itemLabel, mcpEntryJson, showsVersion } from './sync-model';

const stdio: SyncMcpPreview = { name: 'files', transport: 'stdio', command: 'npx', args: ['-y', 'files-mcp'], url: null, enabled: true };
const remote: SyncMcpPreview = { name: 'docs', transport: 'http', command: null, args: [], url: 'https://docs.example/mcp', enabled: true };

describe('mcpEntryJson shows what the import writes', () => {
  it('writes only command and args for a local program', () => {
    expect(mcpEntryJson(stdio)).toBe('"files": {\n  "command": "npx",\n  "args": ["-y", "files-mcp"]\n}');
  });

  it('writes type and url for a remote server', () => {
    expect(mcpEntryJson(remote)).toBe('"docs": {\n  "type": "http",\n  "url": "https://docs.example/mcp"\n}');
    expect(mcpEntryJson({ ...remote, transport: 'sse' })).toContain('"type": "sse"');
  });

  it('adds enabled false last when the server needs values', () => {
    expect(mcpEntryJson({ ...stdio, enabled: false })).toBe('"files": {\n  "command": "npx",\n  "args": ["-y", "files-mcp"],\n  "enabled": false\n}');
    expect(mcpEntryJson({ ...remote, enabled: false })).toMatch(/"url": "https:\/\/docs.example\/mcp",\n {2}"enabled": false\n\}$/);
  });
});

describe('mcpEntryJson for OpenCode shows the shape its file uses', () => {
  it('writes a local server with the command and its arguments in one array', () => {
    expect(mcpEntryJson(stdio, 'opencode')).toBe('"files": {\n  "type": "local",\n  "command": ["npx", "-y", "files-mcp"],\n  "enabled": true\n}');
  });

  it('writes enabled false for a server that needs values, never a placeholder for them', () => {
    const json = mcpEntryJson({ ...stdio, enabled: false }, 'opencode');

    expect(json).toContain('"enabled": false');
    expect(json).not.toMatch(/environment|headers|env/);
  });

  it('writes a remote server with its url, also for sse', () => {
    expect(mcpEntryJson(remote, 'opencode')).toBe('"docs": {\n  "type": "remote",\n  "url": "https://docs.example/mcp",\n  "enabled": true\n}');
    expect(mcpEntryJson({ ...remote, transport: 'sse', enabled: false }, 'opencode')).toBe('"docs": {\n  "type": "remote",\n  "url": "https://docs.example/mcp",\n  "enabled": false\n}');
  });

  it('leaves the OMP and PI shapes as they were', () => {
    expect(mcpEntryJson(stdio, 'omp')).toBe(mcpEntryJson(stdio));
    expect(mcpEntryJson(remote, 'pi')).toBe(mcpEntryJson(remote));
  });
});

describe('itemLabel', () => {
  const plugin = { kind: 'plugin', label: 'wakatime', version: '1.2.3' } as const;

  it('pins an OpenCode plugin in its name and then needs no version line', () => {
    expect(itemLabel('opencode', plugin)).toBe('wakatime@1.2.3');
    expect(showsVersion('opencode', plugin)).toBe(false);
    expect(itemLabel('opencode', { ...plugin, version: null })).toBe('wakatime');
  });

  it('keeps the name of an OMP plugin and shows its version apart', () => {
    expect(itemLabel('omp', plugin)).toBe('wakatime');
    expect(showsVersion('omp', plugin)).toBe(true);
  });
});
