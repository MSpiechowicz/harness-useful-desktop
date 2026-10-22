import { render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

import type { ExtensionAction } from '$lib/ipc/bindings/ExtensionAction';
import type { McpRefusal } from '$lib/ipc/bindings/McpRefusal';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';

import SetupMcpNote from './SetupMcpNote.svelte';
import { commandOf, mcpSnippet, refusalText } from './setup-model';

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

describe('commandOf for Claude Code shows the line that really runs', () => {
  const line = (action: ExtensionAction) => commandOf('claude', action);

  it('adds the user scope and --json to a marketplace and a plugin install', () => {
    expect(line({ type: 'marketplaceAdd', source: 'anthropics/claude-plugins' })).toBe('claude plugin marketplace add anthropics/claude-plugins --scope user --json');
    expect(line({ type: 'pluginInstall', target: 'review@acme', scope: 'user' })).toBe('claude plugin install review@acme --scope user --json');
    expect(line({ type: 'pluginInstall', target: 'review@acme', scope: 'project' })).toBe('claude plugin install review@acme --scope project --json');
  });

  it('never shows the raw name of an action', () => {
    const lines = [
      line({ type: 'pluginUninstall', name: 'review' }),
      line({ type: 'pluginEnable', name: 'review' }),
      line({ type: 'pluginDisable', name: 'review' }),
      line({ type: 'pluginUpgrade', name: 'review' }),
      line({ type: 'marketplaceRemove', name: 'acme' }),
      line({ type: 'marketplaceUpdate', name: 'acme' }),
      line({ type: 'marketplaceUpdate', name: null }),
    ];

    expect(lines).toEqual([
      'claude plugin uninstall review --scope user --json',
      'claude plugin enable review --scope user --json',
      'claude plugin disable review --scope user --json',
      'claude plugin update review --scope user --json',
      'claude plugin marketplace remove acme --scope user --json',
      'claude plugin marketplace update acme --json',
      'claude plugin marketplace update',
    ]);
    expect(lines.join(' ')).not.toMatch(/pluginEnable|pluginDisable|marketplaceRemove/);
  });

  it('leaves the lines of the other harnesses as they were', () => {
    expect(commandOf('omp', { type: 'pluginInstall', target: 'x@y', scope: 'user' })).toBe('omp plugin install x@y --scope user');
    expect(commandOf('omp', { type: 'marketplaceAdd', source: 'o/r' })).toBe('omp plugin marketplace add o/r');
    expect(commandOf('pi', { type: 'packageInstall', source: 'npm:pkg' })).toBe('pi install npm:pkg');
  });
});

describe('a Codex MCP file the app leaves alone', () => {
  it('is TOML, so the refusals and the entry to paste say TOML', () => {
    expect(refusalText('unparsable', 'codex')).toBe('The file is not valid TOML.');
    expect(refusalText('serversNotAnObject', 'codex')).toBe('“mcp_servers” in the file is not a table.');
    expect(refusalText('unparsable', 'claude')).toBe('The file is not valid JSON.');
    expect(mcpSnippet('memoryMcp', 'codex')).toBe('[mcp_servers.harness-memory]\ncommand = "harness-memory"\nargs = ["mcp"]');
  });

  it('shows that entry under the refusal', () => {
    render(SetupMcpNote, { kind: 'desktopMcp', harness: 'codex', title: 'Harness Desktop MCP', refused: 'unparsable', manualEdit: null, disabled: false, waitsFor: null, failure: null });

    expect(screen.getByText(/The file is not valid TOML\./)).toBeInTheDocument();
    expect(screen.getByText(/\[mcp_servers\.harness-desktop\]/)).toBeInTheDocument();
    expect(screen.queryByText(/mcpServers/)).not.toBeInTheDocument();
  });
});

describe('commandOf for Codex shows the line that really runs', () => {
  const line = (action: ExtensionAction) => commandOf('codex', action);

  it('adds plugins by name@marketplace and upgrades marketplaces', () => {
    expect(line({ type: 'pluginInstall', target: 'review@acme', scope: 'user' })).toBe('codex plugin add review@acme');
    expect(line({ type: 'pluginUninstall', name: 'review@acme' })).toBe('codex plugin remove review@acme');
    expect(line({ type: 'marketplaceAdd', source: 'acme/codex-plugins' })).toBe('codex plugin marketplace add acme/codex-plugins');
    expect(line({ type: 'marketplaceRemove', name: 'acme' })).toBe('codex plugin marketplace remove acme');
    expect(line({ type: 'marketplaceUpdate', name: 'acme' })).toBe('codex plugin marketplace upgrade acme');
    expect(line({ type: 'marketplaceUpdate', name: null })).toBe('codex plugin marketplace upgrade');
  });
});

describe('a failed claude mcp run', () => {
  afterEach(() => {
    harness.summaries = [];
  });


  it('is told apart from a file the app could not write', () => {
    expect(refusalText('cliFailed', 'claude')).toBe('The “claude mcp” command failed, so nothing was changed. Run it in a terminal to see why.');
    expect(refusalText('cliFailed', 'claude')).not.toBe(refusalText('writeFailed', 'claude'));
  });

  it('names Codex’s own command for a Codex file', () => {
    expect(refusalText('cliFailed', 'codex')).toBe('The “codex mcp” command failed, so nothing was changed. Run it in a terminal to see why.');
    expect(refusalText('cliRemovedEntry', 'codex')).toMatch(/^The “codex mcp” command removed the app’s entry/);
  });

  it('has a sentence for every refusal', () => {
    const all: Record<McpRefusal, true> = { unparsable: true, notAnObject: true, serversNotAnObject: true, symlink: true, notRegularFile: true, tooLarge: true, unreadable: true, writeFailed: true, cliFailed: true, cliRemovedEntry: true, cliBatchShim: true };

    for (const reason of Object.keys(all) as McpRefusal[]) {
      expect(refusalText(reason, 'claude')).not.toMatch(/^onboarding\./);
    }
  });

  it('offers no entry to paste into the file, since only the CLI changes it', () => {
    const props = { kind: 'memoryMcp', harness: 'claude', title: 'Harness Memory MCP', manualEdit: null, disabled: false, waitsFor: null, failure: null } as const;

    render(SetupMcpNote, { ...props, refused: 'cliFailed' });

    expect(screen.getByText(/“claude mcp” command failed/)).toBeInTheDocument();
    expect(screen.queryByText(/mcpServers/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Add this to the file yourself/)).not.toBeInTheDocument();
  });

  it('says how to add a removed entry back, and to use the native installer for a batch shim, without a snippet', () => {
    const props = { kind: 'desktopMcp', harness: 'claude', title: 'Harness Desktop MCP', manualEdit: null, disabled: false, waitsFor: null, failure: null } as const;

    const removed = render(SetupMcpNote, { ...props, refused: 'cliRemovedEntry' });
    expect(screen.getByText(/Run Recommended setup again to add it\./)).toBeInTheDocument();
    expect(screen.queryByText(/Add this to the file yourself/)).not.toBeInTheDocument();
    removed.unmount();

    render(SetupMcpNote, { ...props, refused: 'cliBatchShim' });
    expect(screen.getByText(/Install it again without npm\./)).toBeInTheDocument();
    expect(screen.queryByText(/mcpServers/)).not.toBeInTheDocument();
  });

  it('names the harness whose CLI is a batch shim, never another one', () => {
    harness.summaries = fixtures.harnesses;

    expect(refusalText('cliBatchShim', 'claude')).toBe('Claude Code installed through npm cannot take this command on Windows. Install it again without npm.');
    expect(refusalText('cliBatchShim', 'codex')).toBe('Codex installed through npm cannot take this command on Windows. Install it again without npm.');
    expect(refusalText('cliBatchShim', 'codex')).not.toMatch(/Claude/);
  });
});
