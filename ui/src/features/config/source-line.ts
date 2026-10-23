import type { SecretKind } from '$lib/ipc/bindings/SecretKind';

/** What a line shows after its key; `close` is a closing brace or bracket on a line of its own, `inline` a list that fits on the line of its key (TOML). */
export type ValueKind = 'null' | 'bool' | 'number' | 'text' | 'secret' | 'map' | 'list' | 'inline' | 'close';

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
  /** A TOML table header (`[a.b]`, `[[a.b]]`): `shownKey` is the whole header and no value follows it. */
  header?: boolean;
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

export function clip(text: string): string {
  return text.length > MAX_TEXT_CHARS ? `${text.slice(0, MAX_TEXT_CHARS)}…` : text;
}
