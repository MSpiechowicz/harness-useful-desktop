import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';

import { displayText } from '$lib/utils/display-text';

import { isJsonObject } from './tools';

/** The harness family servers whose tools get cards of their own. */
export type FamilyServer = 'memory' | 'knowledge';

/**
 * The tool a call name is of one harness family server, whatever the harness calls it: Claude Code and Codex
 * `mcp__harness-memory__note_search`, OpenCode `harness-memory_note_search`, OMP `mcp__harness_memory_note_search`, a Claude plugin
 * install `mcp__plugin_<plugin>_harness-memory__note_search`. The server segment is anchored: after an optional `mcp__` and an
 * optional `plugin_<plugin>_` (a kebab-case plugin name) it is exactly `harness-<server>` (or `harness_<server>`), then the tool.
 * Another server's tool, a bare legacy name and a look-alike server such as `mcp__evil_harness_memory__note_search` are not one.
 * Null for those.
 */
export function harnessToolName(server: FamilyServer, name: string): string | null {
  return new RegExp(`^(?:mcp__)?(?:plugin_[A-Za-z0-9-]+_)?harness[-_]${server}(?:__|_)([a-z][a-z0-9_]*)$`).exec(name)?.[1] ?? null;
}

/** A text argument of a call, trimmed; null when it is missing or blank. */
export function textArg(args: JsonValue | null, name: string): string | null {
  const value = isJsonObject(args) ? args[name] : undefined;

  return typeof value === 'string' && value.trim() !== '' ? value.trim() : null;
}

/** The longest reason a feedback call shows, in characters: what the sidecar keeps of it. */
export const FEEDBACK_REASON_MAX = 200;

/** The reason a feedback call gave: the agent wrote it, so it is cut like the sidecar cuts it and shown without hidden characters. Null when there is none. */
export function reasonArg(args: JsonValue | null): string | null {
  return displayText(textArg(args, 'reason') ?? '', FEEDBACK_REASON_MAX) || null;
}

/** The most rows a card lists. A reply names far fewer, so this bounds what a forged one makes the card draw and look up. */
export const MAX_CARD_ROWS = 50;

/**
 * The ids a `marked N <noun>s useful: …` reply lists, the shape both feedback tools answer in: `marked 2 notes useful: #12, #40`,
 * `marked 1 document useful: 12` or `marked no documents useful`, then optional `not found: …` and `not checked: …` lines for ids
 * that do not count. `ref` is the source of the pattern of one id, `key` turns its text into the id (null when it is not one).
 * Null when the reply is not that (an old server, an error, text cut short), so a caller falls back to what the call asked for.
 */
export function parseMarkedReply<K>(text: string | null, noun: string, ref: string, key: (token: string) => K | null): K[] | null {
  const first = (text ?? '').split('\n', 1)[0].trim();
  const match = new RegExp(`^marked (?:no ${noun}s|(\\d+) ${noun}s?) useful(?:: (${ref}(?:, ${ref})*))?$`).exec(first);
  if (!match) {
    return null;
  }

  if (match[2] === undefined) {
    return match[1] === undefined ? [] : null;
  }

  const ids = match[2].split(', ').map(key);
  if (match[1] === undefined || Number(match[1]) !== ids.length || ids.some((id) => id === null)) {
    return null;
  }

  return [...new Set(ids as K[])].slice(0, MAX_CARD_ROWS);
}
