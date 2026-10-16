import { childrenOf, matchIndices } from '$lib/components/config-tree';
import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import type { ConfigFormat } from '$lib/ipc/bindings/ConfigFormat';
import type { ConfigValue } from '$lib/ipc/bindings/ConfigValue';
import type { SecretKind } from '$lib/ipc/bindings/SecretKind';

/** What a line shows after its key; `close` is a closing brace or bracket on a line of its own. */
export type ValueKind = 'null' | 'bool' | 'number' | 'text' | 'secret' | 'map' | 'list' | 'close';

export interface SourceLine {
  /** 1-based line number in the file as printed here. */
  no: number;
  depth: number;
  /** The key as written in the config, without quotes. */
  key?: string;
  /** The key as printed in the file's format, which is what the line shows and what a search matches. */
  shownKey?: string;
  valueKind: ValueKind;
  /** The value as printed, quoted where the format quotes it; empty where the value is only its children. */
  text: string;
  /** The value without quoting. Scalars only; never a secret. A search matches `text`, the printed form. */
  plain?: string;
  /** Which kind of secret sits here: the chip shows this, never a value. */
  secret?: SecretKind;
  /** A YAML list item marker (`- `) comes first. */
  bullet?: boolean;
  /** A JSON line followed by a sibling ends in a comma. */
  comma?: boolean;
  /** Set on a line that opens a block with entries: the block's id. */
  foldStart?: string;
  /** Entries in the block. */
  foldCount?: number;
  /** Index (in the returned array) of the last line of the block. */
  foldEnd?: number;
}

/** Longest text printed on a line before it is clipped with an ellipsis. */
export const MAX_TEXT_CHARS = 200;

const RESERVED_WORDS = new Set(['true', 'false', 'null', 'yes', 'no', 'on', 'off', 'y', 'n', '~']);

function clip(text: string): string {
  return text.length > MAX_TEXT_CHARS ? `${text.slice(0, MAX_TEXT_CHARS)}…` : text;
}

/** Whether YAML would read the text back as the same string without quotes. */
function plainSafe(text: string): boolean {
  return (
    /^[A-Za-z_./@][\w ./@+:-]*$/.test(text) && !text.endsWith(' ') && !text.endsWith(':') && !text.includes(': ') && !RESERVED_WORDS.has(text.toLowerCase())
  );
}

/** JSON with comments is written like JSON. */
function isJson(format: ConfigFormat): boolean {
  return format === 'json' || format === 'jsonc';
}

function printText(text: string, format: ConfigFormat): string {
  const clipped = clip(text);

  if (!isJson(format) && clipped === text && plainSafe(text)) {
    return text;
  }

  // One line: newlines and quotes become escapes.
  return JSON.stringify(clipped);
}

/** A scalar (or an empty container) as one value on a line. */
function printScalar(value: ConfigValue, format: ConfigFormat): Pick<SourceLine, 'valueKind' | 'text' | 'plain' | 'secret'> {
  const valueKind = value.kind;

  switch (value.kind) {
    case 'text':
      return { valueKind, text: printText(value.value, format), plain: value.value };
    case 'number':
      return { valueKind, text: value.value, plain: value.value };
    case 'bool':
      return { valueKind, text: String(value.value), plain: String(value.value) };
    case 'null':
      return { valueKind, text: 'null', plain: 'null' };
    case 'secret':
      return { valueKind, text: '', secret: value.value };
    case 'map':
      return { valueKind, text: '{}' };
    case 'list':
      return { valueKind, text: '[]' };
  }
}

function printKey(key: string, format: ConfigFormat): string {
  return isJson(format) || !/^[\w.@/-][\w.@/ -]*$/.test(key) ? JSON.stringify(key) : key;
}

interface Entry {
  key?: string;
  value: ConfigValue;
  last: boolean;
  /** Where the entry sits in the tree, for the id of the block it opens. */
  path: string;
}

/**
 * The redacted tree as the lines of a file: JSON with braces and quoted keys for `json` and `jsonc`, YAML for every other
 * format. Secrets are lines that carry only their kind, so no value can reach the screen from here.
 */
export function toLines(fields: ConfigField[], format: ConfigFormat): SourceLine[] {
  const json = isJson(format);
  const lines: SourceLine[] = [];

  function push(line: Omit<SourceLine, 'no'>): number {
    lines.push({ no: lines.length + 1, ...line });

    return lines.length - 1;
  }

  function entries(items: Entry[], depth: number): void {
    items.forEach((item) => entry(item, depth));
  }

  function entry(item: Entry, depth: number): void {
    const { key, value, last, path } = item;
    const children = childrenOf(value);
    const bullet = !json && key === undefined;
    const comma = json && !last;
    const base = { depth, key, shownKey: key === undefined ? undefined : printKey(key, format), bullet: bullet || undefined };

    if (children.length === 0) {
      push({ ...base, ...printScalar(value, format), comma: comma || undefined });

      return;
    }

    const isMap = value.kind === 'map';
    const open = push({
      ...base,
      valueKind: isMap ? 'map' : 'list',
      text: json ? (isMap ? '{' : '[') : '',
      foldStart: path,
      foldCount: children.length,
    });

    entries(
      children.map((child, index) => ({
        key: isMap ? child.key : undefined,
        value: child.value,
        last: index === children.length - 1,
        path: `${path}\u001f${child.key}`,
      })),
      depth + 1,
    );

    if (json) {
      push({ depth, valueKind: 'close', text: isMap ? '}' : ']', comma: comma || undefined });
    }

    lines[open].foldEnd = lines.length - 1;
  }

  if (fields.length === 0) {
    push({ depth: 0, valueKind: 'map', text: '{}' });

    return lines;
  }

  const items = fields.map((field, index) => ({ key: field.key, value: field.value, last: index === fields.length - 1, path: field.key }));

  if (json) {
    push({ depth: 0, valueKind: 'map', text: '{' });
    entries(items, 1);
    push({ depth: 0, valueKind: 'close', text: '}' });
  } else {
    entries(items, 0);
  }

  return lines;
}

/**
 * Whether the (already normalized) needle is in the text the line shows: its printed key, or its printed value when it
 * is a scalar. This is the same text the row highlights, so every match has a visible mark. A secret or a container
 * shows no value to search, so only its key can match, as in the tree.
 */
export function lineMatches(line: SourceLine, needle: string): boolean {
  if (needle === '') {
    return false;
  }

  return matchIndices(line.shownKey ?? '', needle).length > 0 || (line.plain !== undefined && matchIndices(line.text, needle).length > 0);
}

export interface VisibleLine {
  line: SourceLine;
  /** Index of the line in the full array. */
  index: number;
  /** A block opener that is folded: its children are not in the result. */
  folded: boolean;
}

/** Indices (in the full array) of lines matching the needle, in file order. */
export function matchingIndices(lines: SourceLine[], needle: string): number[] {
  return lines.flatMap((line, index) => (lineMatches(line, needle) ? [index] : []));
}

/**
 * The lines on screen. A block in `closed` shows only its first line, unless the needle matches somewhere inside it:
 * a search never hides a match behind a fold.
 */
export function visibleLines(lines: SourceLine[], closed: ReadonlySet<string>, needle: string): VisibleLine[] {
  const matches = matchingIndices(lines, needle);
  const rows: VisibleLine[] = [];

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const end = line.foldEnd ?? index;
    const folded = line.foldStart !== undefined && closed.has(line.foldStart) && !matches.some((match) => match > index && match <= end);

    rows.push({ line, index, folded });

    if (folded) {
      index = end;
    }
  }

  return rows;
}

/** Ids of every block that can fold, for collapse all. */
export function foldIds(lines: SourceLine[]): string[] {
  return lines.flatMap((line) => (line.foldStart === undefined ? [] : [line.foldStart]));
}
