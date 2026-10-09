import { describe, expect, it, vi } from 'vitest';

import { parseQuery, RECENT_GROUP, searchPalette, type PaletteCommand, type PaletteKind } from './palette';

function command(id: string, label: string, kind: PaletteKind, extra: Partial<PaletteCommand> = {}): PaletteCommand {
  return { id, label, kind, group: { nav: 'Navigate', action: 'Actions', session: 'Sessions', file: 'Changes', slash: 'Slash commands', config: 'Config', extension: 'Extensions' }[kind], run: vi.fn(), ...extra };
}

const COMMANDS: PaletteCommand[] = [
  command('go-chat', 'Go to Chat', 'nav'),
  command('go-settings', 'Go to Settings', 'nav'),
  command('new-chat', 'New chat', 'action'),
  command('theme', 'Toggle theme', 'action', { keywords: 'dark light' }),
  command('session-1', 'Why does cargo test hang on CI', 'session', { detail: '1 hour ago · claude' }),
  command('session-2', 'Release checklist', 'session', { detail: 'yesterday · gpt-5' }),
  command('file-1', 'src/lib.rs', 'file', { detail: 'Modified' }),
  command('slash-compact', '/compact', 'slash', { detail: 'Compact the conversation' }),
  command('config-1', 'tools.approvalMode', 'config', { detail: 'Settings · User', keywords: 'write' }),
  command('ext-1', 'demo-plugin', 'extension', { detail: 'Plugin' }),
];

function names(results: ReturnType<typeof searchPalette>): string[][] {
  return results.groups.map((group) => group.entries.map((entry) => entry.command.id));
}

describe('parseQuery', () => {
  it('reads a leading prefix as a mode and strips it', () => {
    expect(parseQuery('>new')).toEqual({ mode: 'action', text: 'new' });
    expect(parseQuery('  /comp')).toEqual({ mode: 'slash', text: 'comp' });
    expect(parseQuery('@cargo')).toEqual({ mode: 'session', text: 'cargo' });
    expect(parseQuery('#lib')).toEqual({ mode: 'file', text: 'lib' });
    expect(parseQuery('plain')).toEqual({ mode: 'all', text: 'plain' });
  });
});

describe('searchPalette', () => {
  it('suggests only views and actions before anything is typed', () => {
    const results = searchPalette(COMMANDS, '');

    expect(names(results)).toEqual([['go-chat', 'go-settings'], ['new-chat', 'theme']]);
  });

  it('puts recent entries first, once, and leaves out those that cannot run', () => {
    const withDisabled = COMMANDS.map((entry) => (entry.id === 'go-settings' ? { ...entry, disabled: true } : entry));
    const results = searchPalette(withDisabled, '', { recents: ['theme', 'go-settings', 'gone', 'go-chat'] });

    expect(results.groups[0].name).toBe(RECENT_GROUP);
    expect(names(results)).toEqual([['theme', 'go-chat'], ['go-settings'], ['new-chat']]);
  });

  it('searches every kind when text is typed, ranking groups by their best hit', () => {
    const results = searchPalette(COMMANDS, 'set');

    expect(results.groups.map((group) => group.name)).toEqual(['Navigate', 'Config']);
    expect(names(results)[0]).toEqual(['go-settings']);
  });

  it('narrows to one kind with a prefix and lists all of it without text', () => {
    expect(names(searchPalette(COMMANDS, '@'))).toEqual([['session-1', 'session-2']]);
    expect(names(searchPalette(COMMANDS, '#'))).toEqual([['file-1']]);
    expect(names(searchPalette(COMMANDS, '/'))).toEqual([['slash-compact']]);
    expect(names(searchPalette(COMMANDS, '>'))).toEqual([['go-chat', 'go-settings'], ['new-chat', 'theme']]);
  });

  it('matches sessions by whole words, not scattered letters', () => {
    expect(names(searchPalette(COMMANDS, '@cargo ci'))).toEqual([['session-1']]);
    expect(names(searchPalette(COMMANDS, '@clst'))).toEqual([]);
  });

  it('still lets abbreviations find actions and views', () => {
    expect(names(searchPalette(COMMANDS, 'gtst'))).toEqual([['go-settings']]);
    expect(names(searchPalette(COMMANDS, '>dark'))).toEqual([['theme']]);
  });

  it('caps each group when searching broadly and says how many were left out', () => {
    const many = Array.from({ length: 9 }, (_, index) => command(`f${index}`, `src/file-${index}.ts`, 'file'));
    const results = searchPalette(many, 'file');

    expect(results.groups[0].entries).toHaveLength(5);
    expect(results.groups[0].hidden).toBe(4);
    expect(searchPalette(many, '#file').groups[0].entries).toHaveLength(9);
  });
});
