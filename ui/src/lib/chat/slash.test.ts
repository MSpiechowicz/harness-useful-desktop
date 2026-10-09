import { describe, expect, it } from 'vitest';

import type { CommandSpec } from '$lib/ipc/bindings/CommandSpec';
import { fixtures } from '$lib/ipc/mock/fixtures';

import { acceptCommand, filterCommands, mergeCatalog, resolveSlash, slashQuery } from './slash';

function spec(name: string, invocation: CommandSpec['invocation'], description = `Does ${name}`): CommandSpec {
  return { name, description, argumentHint: null, source: 'builtin', invocation };
}

const CATALOG: CommandSpec[] = [
  spec('review', { kind: 'promptText' }, 'Review the changes'),
  spec('compact', { kind: 'rpc', action: 'compact' }),
  spec('name', { kind: 'rpc', action: 'setSessionName' }),
  spec('new', { kind: 'rpc', action: 'newSession' }),
  spec('clone', { kind: 'rpc', action: 'clone' }),
  spec('export', { kind: 'rpc', action: 'exportHtml' }),
  spec('session', { kind: 'rpc', action: 'getSessionStats' }),
  spec('copy', { kind: 'rpc', action: 'getLastAssistantText' }),
  spec('abort', { kind: 'rpc', action: 'abort' }),
  spec('model', { kind: 'gui', target: 'modelPicker' }),
  spec('share', { kind: 'unsupported', reason: 'Uploading is not available.' }),
];

describe('slashQuery', () => {
  it('is the partial name while the draft is a lone /token', () => {
    expect(slashQuery('/')).toBe('');
    expect(slashQuery('/rev')).toBe('rev');
    expect(slashQuery('/review src')).toBeNull();
    expect(slashQuery('hello /review')).toBeNull();
    expect(slashQuery('')).toBeNull();
  });
});

describe('filterCommands', () => {
  it('lists everything for a bare slash, capped', () => {
    expect(filterCommands(CATALOG, '', 5)).toHaveLength(5);
  });

  it('ranks name matches first and narrows as you type', () => {
    expect(filterCommands(CATALOG, 'rev').map((command) => command.name)[0]).toBe('review');
    expect(filterCommands(CATALOG, 'comp').map((command) => command.name)).toEqual(['compact']);
    expect(filterCommands(CATALOG, 'zzzz')).toEqual([]);
  });

  it('also finds commands by their description once the query is long enough', () => {
    expect(filterCommands(CATALOG, 'changes').map((command) => command.name)).toEqual(['review']);
  });

  it('works on the real catalogs', () => {
    const names = filterCommands(fixtures.commands.omp, 'sec').map((command) => command.name);

    expect(names).toContain('security');
  });
});

describe('mergeCatalog', () => {
  it('prefers live commands and keeps catalog-only ones', () => {
    const live = [spec('review', { kind: 'promptText' }, 'live description')];
    const merged = mergeCatalog(live, CATALOG);

    expect(merged.filter((command) => command.name === 'review')).toEqual(live);
    expect(merged.some((command) => command.name === 'compact')).toBe(true);
  });

  it('falls back to the catalog before any live commands arrive', () => {
    expect(mergeCatalog(null, CATALOG)).toEqual(CATALOG);
  });
});

describe('picking a command from the list', () => {
  const pick = (name: string) => acceptCommand(CATALOG.find((command) => command.name === name) as CommandSpec);

  it('inserts promptText commands into the composer', () => {
    expect(pick('review')).toEqual({ type: 'insert', text: '/review ' });
  });

  it('maps each rpc kind to its action', () => {
    expect(pick('new')).toMatchObject({ type: 'action', action: { type: 'newSession' } });
    expect(pick('clone')).toMatchObject({ type: 'action', action: { type: 'clone' } });
    expect(pick('export')).toMatchObject({ type: 'action', action: { type: 'exportHtml' } });
    expect(pick('session')).toMatchObject({ type: 'action', action: { type: 'getSessionStats' } });
    expect(pick('copy')).toMatchObject({ type: 'action', action: { type: 'getLastAssistantText' } });
    expect(pick('abort')).toMatchObject({ type: 'action', action: { type: 'abort' } });
  });

  it('asks for the text compact and name need', () => {
    expect(pick('compact')).toEqual({ type: 'ask', kind: 'compact' });
    expect(pick('name')).toEqual({ type: 'ask', kind: 'name' });
  });

  it('opens a view for gui commands and explains unsupported ones', () => {
    expect(pick('model')).toEqual({ type: 'gui', target: 'modelPicker' });
    expect(pick('share')).toEqual({ type: 'unsupported', reason: 'Uploading is not available.' });
  });
});

describe('sending a draft that starts with /', () => {
  it('passes arguments to the commands that take them', () => {
    expect(resolveSlash('/compact keep the parser notes', CATALOG)).toMatchObject({ action: { type: 'compact', instructions: 'keep the parser notes' } });
    expect(resolveSlash('/name Parser work', CATALOG)).toMatchObject({ action: { type: 'setSessionName', name: 'Parser work' } });
  });

  it('runs compact without instructions when none were typed', () => {
    expect(resolveSlash('/compact', CATALOG)).toMatchObject({ action: { type: 'compact', instructions: null } });
  });

  it('sends promptText commands to the harness as typed', () => {
    expect(resolveSlash('/review src/lib.rs', CATALOG)).toEqual({ type: 'send', text: '/review src/lib.rs' });
  });

  it('sends unknown commands through so the harness can decide', () => {
    expect(resolveSlash('/mystery arg', CATALOG)).toEqual({ type: 'send', text: '/mystery arg' });
    expect(resolveSlash('not a command', CATALOG)).toEqual({ type: 'send', text: 'not a command' });
  });
});
