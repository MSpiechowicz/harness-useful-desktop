import { describe, expect, it } from 'vitest';

import type { SyncMcpPreview } from '$lib/ipc/bindings/SyncMcpPreview';

import { mcpEntryJson } from './sync-model';

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
