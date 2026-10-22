import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { SyncMcpPreview } from '$lib/ipc/bindings/SyncMcpPreview';
import type { SyncNote } from '$lib/ipc/bindings/SyncNote';
import type { SyncStep } from '$lib/ipc/bindings/SyncStep';

import SyncNotices from './SyncNotices.svelte';
import { addedByCli, cliAddCommand, failedCliNote, itemLabel, keysLine, mcpEntryJson, showsVersion } from './sync-model';

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

describe('mcpEntryJson for OpenCode 2 has no enabled', () => {
  it('writes a local server and a remote one without it', () => {
    expect(mcpEntryJson(stdio, 'opencode', 2)).toBe('"files": {\n  "type": "local",\n  "command": ["npx", "-y", "files-mcp"]\n}');
    expect(mcpEntryJson(remote, 'opencode', 2)).toBe('"docs": {\n  "type": "remote",\n  "url": "https://docs.example/mcp"\n}');
  });

  it('writes disabled true for a server that needs values', () => {
    expect(mcpEntryJson({ ...stdio, enabled: false }, 'opencode', 2)).toBe('"files": {\n  "type": "local",\n  "command": ["npx", "-y", "files-mcp"],\n  "disabled": true\n}');
  });

  it('leaves the OMP and PI shapes alone', () => {
    expect(mcpEntryJson(stdio, 'omp', 2)).toBe(mcpEntryJson(stdio));
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

describe('an MCP server that Claude Code adds through its CLI', () => {
  const cliStep: SyncStep = { type: 'cli', command: ['claude', 'mcp', 'add-json', '--scope', 'user', 'files', '{}'] };
  const notes: SyncNote[] = [];
  const props = { runsProgram: false, envKeys: ['API_KEY'], headerKeys: ['Authorization'], notes, viaCli: { command: 'claude mcp add-json', harness: 'Claude Code' } };

  it('is recognised by its command step, and only for an MCP server', () => {
    expect(addedByCli({ kind: 'mcpServer', steps: [cliStep] })).toBe(true);
    expect(addedByCli({ kind: 'plugin', steps: [cliStep] })).toBe(false);
    expect(addedByCli({ kind: 'mcpServer', steps: [{ type: 'mcpMerge', file: '~/.omp/mcp.json', server: stdio }] })).toBe(false);
  });

  it('says the server is added with claude mcp add-json', () => {
    render(SyncNotices, { ...props, envKeys: [], headerKeys: [] });

    expect(screen.getByText('Added with “claude mcp add-json”.')).toBeInTheDocument();
    expect(screen.queryByText(/Needs/)).not.toBeInTheDocument();
  });

  it('says the values of the listed keys are set in Claude Code afterwards', () => {
    render(SyncNotices, props);

    expect(screen.getByText('Needs API_KEY, Authorization. Set the values in Claude Code after importing.')).toBeInTheDocument();
    expect(screen.getByText('Added with “claude mcp add-json”.')).toBeInTheDocument();
  });

  it('keeps the wording of a file-based harness', () => {
    expect(keysLine(['API_KEY'], [])).toBe('Needs API_KEY. Set it after importing.');
    expect(keysLine([], [], 'Claude Code')).toBeNull();

    render(SyncNotices, { ...props, viaCli: null });

    expect(screen.queryByText(/claude mcp add-json/)).not.toBeInTheDocument();
  });

  it('names the failed command above the CLI summary', () => {
    const argv = ['claude', 'mcp', 'add-json', '--scope', 'user', 'files', '{}'];
    const failed: Parameters<typeof failedCliNote>[0] = { kind: 'mcpServer', outcome: { type: 'failed' }, argv };

    expect(failedCliNote(failed)).toBe('The “claude mcp” command failed.');
    expect(failedCliNote({ ...failed, outcome: { type: 'done' } })).toBeNull();
    expect(failedCliNote({ ...failed, kind: 'plugin' })).toBeNull();
    expect(failedCliNote({ ...failed, argv: ['omp', 'plugin', 'install'] })).toBeNull();
  });

  it('names Codex’s own command for a server Codex adds', () => {
    const codexStep: SyncStep = { type: 'cli', command: ['codex', 'mcp', 'add', 'files', '--', 'files-server'] };

    expect(cliAddCommand({ kind: 'mcpServer', steps: [codexStep] })).toBe('codex mcp add');
    expect(failedCliNote({ kind: 'mcpServer', outcome: { type: 'failed' }, argv: codexStep.command })).toBe('The “codex mcp” command failed.');
    expect(keysLine(['FILES_TOKEN'], [], 'Codex')).toBe('Needs FILES_TOKEN. Set the values in Codex after importing.');
  });
});
