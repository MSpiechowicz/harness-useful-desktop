import { describe, expect, it } from 'vitest';

import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import type { ConfigValue } from '$lib/ipc/bindings/ConfigValue';

import { lineMatches, MAX_TEXT_CHARS, toLines, visibleLines, type SourceLine } from './source';

const text = (value: string): ConfigValue => ({ kind: 'text', value });
const map = (value: ConfigField[]): ConfigValue => ({ kind: 'map', value });
const list = (value: ConfigValue[]): ConfigValue => ({ kind: 'list', value });
const field = (key: string, value: ConfigValue): ConfigField => ({ key, value });

/** The line as the file would read it: headers and values as written, `key = value` for entries. */
function print(lines: SourceLine[]): string[] {
  return lines.map((line) => {
    const indent = '  '.repeat(line.depth);

    if (line.header) {
      return line.shownKey!;
    }

    const key = line.shownKey === undefined ? '' : `${line.shownKey} = `;
    const value = line.secret === undefined ? line.text : '<secret>';

    return `${indent}${key}${value}${line.comma ? ',' : ''}`;
  });
}

const CODEX: ConfigField[] = [
  field('model', text('gpt-5.6-terra')),
  field('mcp_servers', map([
    field('memory', map([field('command', text('harness-memory')), field('args', list([text('serve'), text('--stdio')]))])),
    field('docs', map([field('url', text('https://example.com/mcp')), field('bearer_token_env_var', { kind: 'secret', value: { type: 'envRef', name: 'DOCS_TOKEN' } })])),
  ])),
  field('approval_policy', text('on-request')),
  field('projects', map([field('/home/me/my.project', map([field('trust_level', text('trusted'))]))])),
  field('features', map([field('in_app_updates', { kind: 'bool', value: false })])),
];

describe('toLines as TOML', () => {
  it('writes keys as `key = value` first and each nested map as a table after them', () => {
    expect(print(toLines(CODEX, 'toml'))).toEqual([
      'model = "gpt-5.6-terra"',
      'approval_policy = "on-request"',
      '',
      '[mcp_servers.memory]',
      'command = "harness-memory"',
      'args = ["serve", "--stdio"]',
      '',
      '[mcp_servers.docs]',
      'url = "https://example.com/mcp"',
      'bearer_token_env_var = <secret>',
      '',
      '[projects."/home/me/my.project"]',
      'trust_level = "trusted"',
      '',
      '[features]',
      'in_app_updates = false',
    ]);
  });

  it('skips the header of a table that only holds tables, and quotes a key that is not bare', () => {
    const lines = toLines(CODEX, 'toml');

    expect(lines.filter((line) => line.header).map((line) => line.shownKey)).toEqual([
      '[mcp_servers.memory]',
      '[mcp_servers.docs]',
      '[projects."/home/me/my.project"]',
      '[features]',
    ]);
  });

  it('numbers the lines from one and folds a table up to its last line', () => {
    const lines = toLines(CODEX, 'toml');
    const memory = lines.find((line) => line.shownKey === '[mcp_servers.memory]')!;

    expect(lines.map((line) => line.no)).toEqual(Array.from({ length: lines.length }, (_, index) => index + 1));
    expect(memory).toMatchObject({ foldCount: 2, foldEnd: memory.no + 1 });

    const rows = visibleLines(lines, new Set([memory.foldStart!]), '');

    expect(rows.find((row) => row.line.no === memory.no)?.folded).toBe(true);
    expect(rows.some((row) => row.line.key === 'command')).toBe(false);
    expect(rows.some((row) => row.line.key === 'url')).toBe(true);
  });

  it('writes a list of maps as repeated double-bracket tables', () => {
    const lines = toLines([field('hooks', map([field('PreToolUse', list([map([field('matcher', text('Bash'))]), map([field('matcher', text('apply_patch'))])]))]))], 'toml');

    expect(print(lines)).toEqual(['[[hooks.PreToolUse]]', 'matcher = "Bash"', '', '[[hooks.PreToolUse]]', 'matcher = "apply_patch"']);
    expect(lines[0].foldStart).not.toBe(lines[3].foldStart);
  });

  it('keeps the double-bracket header of a list element that holds only tables', () => {
    const element = (command: string) => map([field('hooks', list([map([field('command', text(command))])]))]);
    const lines = toLines([field('hooks', map([field('PreToolUse', list([element('lint'), element('test')]))]))], 'toml');

    expect(print(lines)).toEqual([
      '[[hooks.PreToolUse]]',
      '',
      '[[hooks.PreToolUse.hooks]]',
      'command = "lint"',
      '',
      '[[hooks.PreToolUse]]',
      '',
      '[[hooks.PreToolUse.hooks]]',
      'command = "test"',
    ]);
  });

  it('writes a list that holds a secret over several lines, and a list inside it on one line when it fits', () => {
    const lines = toLines(
      [field('args', list([text('--token'), { kind: 'secret', value: { type: 'envRef', name: 'T' } }])), field('grid', list([list([{ kind: 'number', value: '1' }])]))],
      'toml',
    );

    expect(print(lines)).toEqual(['args = [', '  "--token",', '  <secret>', ']', 'grid = [', '  [1]', ']']);
    expect(lines[0]).toMatchObject({ foldCount: 2, foldEnd: 3 });
  });

  it('keeps a secret as its kind and quotes text, clipping a long one', () => {
    const lines = toLines([field('a', text('x\ny')), field('b', text('x'.repeat(MAX_TEXT_CHARS + 20))), field('c', { kind: 'secret', value: { type: 'envRef', name: 'K' } })], 'toml');

    expect(lines[0].text).toBe('"x\\ny"');
    expect(lines[1].text).toBe(`"${'x'.repeat(MAX_TEXT_CHARS)}…"`);
    expect(lines[2]).toMatchObject({ valueKind: 'secret', text: '', secret: { type: 'envRef', name: 'K' } });
    expect(lines[2].plain).toBeUndefined();
  });

  it('prints empty containers inline and an empty file as an empty table', () => {
    expect(print(toLines([field('a', map([])), field('b', list([]))], 'toml'))).toEqual(['a = {}', 'b = []']);
    expect(print(toLines([], 'toml'))).toEqual(['{}']);
  });

  it('matches a header by its table name and an inline list by what it shows', () => {
    const lines = toLines(CODEX, 'toml');
    const header = lines.find((line) => line.shownKey === '[mcp_servers.docs]')!;
    const args = lines.find((line) => line.key === 'args')!;

    expect(lineMatches(header, 'servers.docs')).toBe(true);
    expect(lineMatches(args, '--stdio')).toBe(true);
    expect(lineMatches(lines.find((line) => line.secret !== undefined)!, 'docs_token')).toBe(false);
  });
});
