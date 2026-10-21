import { describe, expect, it } from 'vitest';

import { mcpSnippet } from './setup-model';

describe('mcpSnippet', () => {
  it('shows OpenCode 1 the entry under "mcp" with enabled', () => {
    const entry = JSON.parse(mcpSnippet('memoryMcp', 'opencode'));

    expect(entry).toEqual({ mcp: { 'harness-memory': { type: 'local', command: ['harness-memory', 'mcp'], enabled: true } } });
    expect(mcpSnippet('memoryMcp', 'opencode', 1)).toBe(mcpSnippet('memoryMcp', 'opencode'));
  });

  it('shows OpenCode 2 the entry under "mcp.servers" without enabled', () => {
    const entry = JSON.parse(mcpSnippet('desktopMcp', 'opencode', 2));

    expect(entry).toEqual({ mcp: { servers: { 'harness-desktop': { type: 'local', command: ['harness-desktop', 'mcp'] } } } });
  });

  it('keeps the OMP and PI shape whatever the OpenCode generation', () => {
    const entry = { mcpServers: { 'harness-dashboard': { command: 'harness-dashboard', args: ['mcp'] } } };

    expect(JSON.parse(mcpSnippet('mcp', 'omp', 2))).toEqual(entry);
    expect(JSON.parse(mcpSnippet('mcp', 'pi'))).toEqual(entry);
  });
});
