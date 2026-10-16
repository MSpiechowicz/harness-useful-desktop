import { describe, expect, it } from 'vitest';

import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import type { ConfigValue } from '$lib/ipc/bindings/ConfigValue';

import { foldIds, lineMatches, MAX_TEXT_CHARS, toLines, visibleLines, type SourceLine } from './source';

const text = (value: string): ConfigValue => ({ kind: 'text', value });
const map = (value: ConfigField[]): ConfigValue => ({ kind: 'map', value });
const field = (key: string, value: ConfigValue): ConfigField => ({ key, value });

const FIELDS: ConfigField[] = [
  field('name', text('omp')),
  field('retries', { kind: 'number', value: '3' }),
  field('theme', map([field('dark', text('titanium')), field('light', { kind: 'null' })])),
  field('tags', { kind: 'list', value: [text('a b'), { kind: 'bool', value: true }] }),
  field('empty', map([])),
];

/** One printable string per line, the way the file would read. */
function print(lines: SourceLine[], quote = false): string[] {
  return lines.map((line) => {
    const key = line.key === undefined ? '' : `${quote ? JSON.stringify(line.key) : line.key}:`;
    const parts = [line.bullet ? '-' : '', key, line.text].filter(Boolean).join(' ');

    return `${'  '.repeat(line.depth)}${parts}${line.comma ? ',' : ''}`;
  });
}

describe('toLines as YAML', () => {
  it('writes keys, nested maps and list items', () => {
    expect(print(toLines(FIELDS, 'yaml'))).toEqual([
      'name: omp',
      'retries: 3',
      'theme:',
      '  dark: titanium',
      '  light: null',
      'tags:',
      '  - a b',
      '  - true',
      'empty: {}',
    ]);
  });

  it('uses the same style for markdown and text', () => {
    expect(toLines(FIELDS, 'markdown')).toEqual(toLines(FIELDS, 'yaml'));
    expect(toLines(FIELDS, 'text')).toEqual(toLines(FIELDS, 'yaml'));
  });

  it('quotes text that would read back as another type and clips long text', () => {
    const lines = toLines(
      [field('a', text('true')), field('b', text('12')), field('c', text('x\ny')), field('d', text('x'.repeat(MAX_TEXT_CHARS + 50))), field('e', text('openai/gpt:high'))],
      'yaml',
    );

    expect(lines.map((line) => line.text)).toEqual(['"true"', '"12"', '"x\\ny"', `"${'x'.repeat(MAX_TEXT_CHARS)}…"`, 'openai/gpt:high']);
  });

  it('numbers lines from one and records each block with its entries and last line', () => {
    const lines = toLines(FIELDS, 'yaml');
    const theme = lines[2];

    expect(lines.map((line) => line.no)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9]);
    expect(theme).toMatchObject({ foldStart: 'theme', foldCount: 2, foldEnd: 4 });
    expect(lines[5]).toMatchObject({ foldStart: 'tags', foldCount: 2, foldEnd: 7 });
    expect(lines[8].foldStart).toBeUndefined();
  });
});

describe('toLines as JSONC', () => {
  it('prints exactly like JSON, so an OpenCode file reads as the JSON it is', () => {
    expect(toLines(FIELDS, 'jsonc')).toEqual(toLines(FIELDS, 'json'));
  });
});

describe('toLines as JSON', () => {
  it('wraps in braces, quotes keys and strings and puts commas between siblings only', () => {
    expect(print(toLines(FIELDS, 'json'), true)).toEqual([
      '{',
      '  "name": "omp",',
      '  "retries": 3,',
      '  "theme": {',
      '    "dark": "titanium",',
      '    "light": null',
      '  },',
      '  "tags": [',
      '    "a b",',
      '    true',
      '  ],',
      '  "empty": {}',
      '}',
    ]);
  });

  it('closes a block on a line of its own and folds up to it', () => {
    const lines = toLines(FIELDS, 'json');

    expect(lines[3]).toMatchObject({ key: 'theme', foldStart: 'theme', foldEnd: 6, foldCount: 2 });
    expect(lines[6]).toMatchObject({ valueKind: 'close', text: '}', comma: true });
  });

  it('prints an empty file as an empty object', () => {
    expect(print(toLines([], 'json'))).toEqual(['{}']);
  });
});

describe('secrets', () => {
  const secret: ConfigField[] = [field('apiKey', { kind: 'secret', value: { type: 'envRef', name: 'OPENAI_API_KEY' } })];

  it('carry only their kind', () => {
    const [line] = toLines(secret, 'yaml');

    expect(line).toMatchObject({ key: 'apiKey', valueKind: 'secret', text: '', secret: { type: 'envRef', name: 'OPENAI_API_KEY' } });
    expect(line.plain).toBeUndefined();
  });

  it('never match by value', () => {
    const [line] = toLines(secret, 'yaml');

    expect(lineMatches(line, 'apikey')).toBe(true);
    expect(lineMatches(line, 'openai')).toBe(false);
  });
});

describe('lineMatches', () => {
  const only = (fields: ConfigField[], format: 'yaml' | 'json' = 'yaml') => toLines(fields, format).find((line) => line.key !== undefined)!;

  it('matches the value as it is printed, so an escaped newline or quote can be found', () => {
    const line = only([field('note', text('say "hi"\nthen leave'))]);

    expect(line.text).toBe('"say \\"hi\\"\\nthen leave"');
    expect(lineMatches(line, '\\n')).toBe(true);
    expect(lineMatches(line, '\\"hi')).toBe(true);
    expect(lineMatches(line, '"hi"\nthen')).toBe(false);
  });

  it('does not match text that is clipped away, which has no mark to show', () => {
    const line = only([field('long', text(`${'a'.repeat(MAX_TEXT_CHARS)}needle`))]);

    expect(line.text.endsWith('…"')).toBe(true);
    expect(lineMatches(line, 'needle')).toBe(false);
    expect(lineMatches(line, 'aaa')).toBe(true);
  });

  it('matches the key as printed, quotes included in JSON', () => {
    expect(lineMatches(only([field('name', text('x'))], 'json'), '"name"')).toBe(true);
    expect(lineMatches(only([field('name', text('x'))], 'yaml'), '"name"')).toBe(false);
  });

  it('matches a key with a space by the quoted form it is shown in', () => {
    expect(lineMatches(only([field('my key', text('x'))]), 'my key')).toBe(true);
  });

  it('matches only the key of a container, never its braces', () => {
    const [open] = toLines([field('theme', map([field('a', text('b'))]))], 'json').filter((line) => line.key === 'theme');

    expect(open.text).toBe('{');
    expect(lineMatches(open, '{')).toBe(false);
    expect(lineMatches(open, 'theme')).toBe(true);
  });

  it('never matches a closing brace line', () => {
    const lines = toLines([field('a', map([field('b', text('c'))]))], 'json');

    expect(lines.filter((line) => lineMatches(line, '}'))).toEqual([]);
  });
});

describe('visibleLines', () => {
  const lines = toLines(FIELDS, 'yaml');

  it('shows everything when nothing is folded', () => {
    expect(visibleLines(lines, new Set(), '')).toHaveLength(lines.length);
  });

  it('hides the inside of a folded block and marks its opener', () => {
    const rows = visibleLines(lines, new Set(['theme']), '');

    expect(rows.map((row) => row.line.no)).toEqual([1, 2, 3, 6, 7, 8, 9]);
    expect(rows[2].folded).toBe(true);
  });

  it('opens a folded block that holds a match while searching', () => {
    const rows = visibleLines(lines, new Set(['theme', 'tags']), 'titanium');

    expect(rows.map((row) => row.line.no)).toEqual([1, 2, 3, 4, 5, 6, 9]);
    expect(rows.filter((row) => row.folded).map((row) => row.line.foldStart)).toEqual(['tags']);
  });

  it('lists every block that can fold', () => {
    expect(foldIds(lines)).toEqual(['theme', 'tags']);
  });
});
