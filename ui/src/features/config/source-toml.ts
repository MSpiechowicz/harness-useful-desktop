import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import type { ConfigValue } from '$lib/ipc/bindings/ConfigValue';

import { clip, type SourceLine } from './source-line';

type Draft = Omit<SourceLine, 'no'>;

/** A list of plain scalars is written on one line while it stays within this many characters. */
const INLINE_LIST_CHARS = 100;

const BARE_KEY = /^[A-Za-z0-9_-]+$/;

/** A key as TOML writes it: bare, else quoted. */
function tomlKey(key: string): string {
  return BARE_KEY.test(key) ? key : JSON.stringify(key);
}

/** A scalar (or an empty container) as one value on a line. Text is always quoted, as TOML requires. */
function scalar(value: ConfigValue): Pick<Draft, 'valueKind' | 'text' | 'plain' | 'secret'> {
  switch (value.kind) {
    case 'text':
      return { valueKind: 'text', text: JSON.stringify(clip(value.value)), plain: value.value };
    case 'number':
      return { valueKind: 'number', text: value.value, plain: value.value };
    case 'bool':
      return { valueKind: 'bool', text: String(value.value), plain: String(value.value) };
    case 'null':
      return { valueKind: 'null', text: 'null', plain: 'null' };
    case 'secret':
      return { valueKind: 'secret', text: '', secret: value.value };
    case 'map':
      return { valueKind: 'map', text: '{}' };
    case 'list':
      return { valueKind: 'list', text: '[]' };
  }
}

function isMapWithEntries(value: ConfigValue): value is Extract<ConfigValue, { kind: 'map' }> {
  return value.kind === 'map' && value.value.length > 0;
}

/** A non-empty list of non-empty maps: TOML writes it as repeated `[[name]]` tables. */
function isTableArray(value: ConfigValue): value is Extract<ConfigValue, { kind: 'list' }> {
  return value.kind === 'list' && value.value.length > 0 && value.value.every(isMapWithEntries);
}

/** The one-line form of a list of plain scalars, or null when it holds anything else or runs long. */
function inlineList(value: ConfigValue): string | null {
  if (value.kind !== 'list' || value.value.length === 0 || !value.value.every((item) => ['text', 'number', 'bool'].includes(item.kind))) {
    return null;
  }

  const text = `[${value.value.map((item) => scalar(item).text).join(', ')}]`;

  return text.length <= INLINE_LIST_CHARS ? text : null;
}

/**
 * The redacted tree as a `config.toml` reads: `key = value` lines first, then one `[table]` per nested map (`[[table]]` per
 * map of a list of maps), each folding with the tables below it. A list that does not fit on a line, and anything inside a
 * list, is written over several lines. Secrets are lines that carry only their kind, so no value can reach the screen from here.
 */
export function toTomlLines(fields: ConfigField[]): SourceLine[] {
  const lines: SourceLine[] = [];

  function push(line: Draft): number {
    lines.push({ no: lines.length + 1, ...line });

    return lines.length - 1;
  }

  /** A key or list item with its value; a container with entries opens a block closed on a line of its own. */
  function valueLines(key: string | undefined, value: ConfigValue, depth: number, id: string, comma: boolean): void {
    const base = { depth, key, shownKey: key === undefined ? undefined : tomlKey(key), comma: comma || undefined };
    const oneLine = inlineList(value);

    if (oneLine !== null) {
      push({ ...base, valueKind: 'inline', text: oneLine, plain: oneLine });

      return;
    }

    if (value.kind !== 'map' && value.kind !== 'list') {
      push({ ...base, ...scalar(value) });

      return;
    }

    const children: ConfigField[] =
      value.kind === 'map' ? value.value : value.value.map((item, index) => ({ key: String(index), value: item }));

    if (children.length === 0) {
      push({ ...base, ...scalar(value) });

      return;
    }

    const isMap = value.kind === 'map';
    const open = push({ ...base, valueKind: isMap ? 'map' : 'list', text: isMap ? '{' : '[', comma: undefined, foldStart: id, foldCount: children.length });

    children.forEach((child, index) => {
      valueLines(isMap ? child.key : undefined, child.value, depth + 1, `${id}\u001f${child.key}`, index < children.length - 1);
    });

    push({ depth, valueKind: 'close', text: isMap ? '}' : ']', comma: comma || undefined });
    lines[open].foldEnd = lines.length - 1;
  }

  /** A `[path]` header (`[[path]]` for a list element) and the table under it. */
  function table(path: string[], entries: ConfigField[], id: string, array: boolean): void {
    const hasOwnLines = entries.some((entry) => !isMapWithEntries(entry.value) && !isTableArray(entry.value));

    if (hasOwnLines) {
      const shown = path.map(tomlKey).join('.');

      if (lines.length > 0) {
        push({ depth: 0, valueKind: 'close', text: '' });
      }

      const open = push({
        depth: 0,
        key: path.join('.'),
        shownKey: array ? `[[${shown}]]` : `[${shown}]`,
        header: true,
        valueKind: 'map',
        text: '',
        foldStart: id,
        foldCount: entries.length,
      });

      body(path, entries, id);
      lines[open].foldEnd = lines.length - 1;

      return;
    }

    body(path, entries, id);
  }

  /** The entries that fit on a line first (TOML needs them before any table below), then the tables. */
  function body(path: string[], entries: ConfigField[], id: string): void {
    const tables = entries.filter((entry) => isMapWithEntries(entry.value) || isTableArray(entry.value));

    entries
      .filter((entry) => !tables.includes(entry))
      .forEach((entry) => valueLines(entry.key, entry.value, 0, `${id}\u001f${entry.key}`, false));

    for (const entry of tables) {
      const next = [...path, entry.key];
      const nextId = `${id}\u001f${entry.key}`;

      if (isMapWithEntries(entry.value)) {
        table(next, entry.value.value, nextId, false);
      } else if (isTableArray(entry.value)) {
        entry.value.value.forEach((item, index) => {
          if (item.kind === 'map') {
            table(next, item.value, `${nextId}\u001f${index}`, true);
          }
        });
      }
    }
  }

  if (fields.length === 0) {
    push({ depth: 0, valueKind: 'map', text: '{}' });

    return lines;
  }

  body([], fields, '');

  return lines;
}
