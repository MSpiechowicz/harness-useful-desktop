import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import type { ConfigValue } from '$lib/ipc/bindings/ConfigValue';
import { childrenOf, isLongText, matchesDeep, matchIndices, normalizeNeedle } from '$lib/components/config-tree';

export type GroupId = 'modelRoles' | 'appearance' | 'behavior' | 'memory' | 'developer' | 'other';

/** Cards in the order they appear. */
export const GROUP_ORDER: readonly GroupId[] = ['modelRoles', 'appearance', 'behavior', 'memory', 'developer', 'other'];

/** Top-level keys whose meaning is known; every other key lands in "other". */
const KEY_GROUPS: Readonly<Record<string, GroupId>> = {
  modelRoles: 'modelRoles',
  theme: 'appearance',
  symbolPreset: 'appearance',
  composer: 'appearance',
  hideThinkingBlock: 'behavior',
  defaultThinkingLevel: 'behavior',
  setupVersion: 'behavior',
  task: 'behavior',
  memory: 'memory',
  mnemopi: 'memory',
  dev: 'developer',
  codexResets: 'developer',
};

export function groupOf(key: string): GroupId {
  return Object.hasOwn(KEY_GROUPS, key) ? KEY_GROUPS[key] : 'other';
}

const THINKING_LEVELS = ['off', 'none', 'minimal', 'low', 'medium', 'high', 'xhigh'] as const;

/** Filled bars of the thinking meter; levels below "low" show an empty meter, and Claude Code's "max" fills it like "xhigh". */
const METER_BARS: Readonly<Record<string, number>> = { low: 1, medium: 2, high: 3, xhigh: 4, max: 4 };

export const METER_SIZE = 4;

/** A Claude Code model ID or alias, as Harness Skills accepts them in Claude Code's `models.yml`. */
const CLAUDE_MODEL = /^(inherit|opus|sonnet|haiku|fable|claude-[a-z0-9][a-z0-9.-]{0,80})$/;

const CLAUDE_EFFORTS = ['low', 'medium', 'high', 'xhigh', 'max'] as const;

/**
 * A Codex model ID, as Harness Skills accepts it in Codex's `models.yml`: any ID of word characters and `. : / @ + -`. A
 * `:suffix` that is not one of {@link CODEX_EFFORTS} is part of the ID.
 */
const CODEX_MODEL = /^[A-Za-z0-9](?:[A-Za-z0-9._:/@+-]{0,198}[A-Za-z0-9._@+-])?$/;

const CODEX_EFFORTS = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh'] as const;

export interface ModelRef {
  /** The provider, or null for a Claude Code selector, which names none. */
  provider: string | null;
  model: string;
  /** The thinking level (a Claude Code effort), or null when the reference names none. */
  level: string | null;
}

/** Parses `provider/model[:level]`, or without a slash a Claude Code or Codex selector `model[:effort]`; null when the text has none of these shapes. */
export function parseModelRef(text: string): ModelRef | null {
  if (!text.includes('/')) {
    return parseClaudeSelector(text) ?? parseCodexSelector(text);
  }

  const slash = text.indexOf('/');
  if (slash === 0 || /\s/.test(text)) {
    return null;
  }

  const provider = text.slice(0, slash);
  let model = text.slice(slash + 1);
  let level: string | null = null;

  const colon = model.lastIndexOf(':');
  const suffix = model.slice(colon + 1);
  if (colon > 0 && (THINKING_LEVELS as readonly string[]).includes(suffix)) {
    level = suffix;
    model = model.slice(0, colon);
  }

  return model === '' ? null : { provider, model, level };
}

/** `model[:effort]` with a Claude model ID or alias (`opus`, `claude-opus-5-5:high`, `inherit`); null for anything else. */
function parseClaudeSelector(text: string): ModelRef | null {
  const [model, effort, extra] = text.split(':');

  if (extra !== undefined || !CLAUDE_MODEL.test(model)) {
    return null;
  }

  if (effort !== undefined && !(CLAUDE_EFFORTS as readonly string[]).includes(effort)) {
    return null;
  }

  return { provider: null, model, level: effort ?? null };
}

/**
 * `model[:effort]` with a Codex model ID (`gpt-5.5:high`), `inherit` (the Codex setting) or `inherit:effort`; null for
 * anything else. Claude's own models and efforts are read by {@link parseClaudeSelector} first, so a Claude alias with an
 * effort Claude Code does not know (`opus:extreme`) stays unparsed rather than turning into a Codex model ID.
 */
function parseCodexSelector(text: string): ModelRef | null {
  const colon = text.lastIndexOf(':');
  const suffix = colon > 0 ? text.slice(colon + 1) : '';
  const level = (CODEX_EFFORTS as readonly string[]).includes(suffix) ? suffix : null;
  const model = level === null ? text : text.slice(0, colon);

  if (model === 'inherit') {
    return { provider: null, model, level };
  }

  if (!CODEX_MODEL.test(model) || CLAUDE_MODEL.test(model.split(':')[0])) {
    return null;
  }

  return { provider: null, model, level };
}

/** How many of the four bars a thinking level fills. */
export function meterBars(level: string): number {
  return METER_BARS[level] ?? 0;
}

export interface RoleRow {
  role: string;
  ref: ModelRef;
  /** The reference as written. */
  text: string;
}

/** The pieces a role row shows, which are what a search matches and highlights: the role, `provider/` (when named), the model, the level. */
export function roleSegments(row: RoleRow): string[] {
  const provider = row.ref.provider === null ? [] : [`${row.ref.provider}/`];

  return [row.role, ...provider, row.ref.model, ...(row.ref.level === null ? [] : [row.ref.level])];
}

/** Whether the (already normalized) needle is inside one of the pieces the row shows. */
export function roleMatches(row: RoleRow, needle: string): boolean {
  return roleSegments(row).some((segment) => matchIndices(segment, needle).length > 0);
}

/** The roles of a `modelRoles` map with `default` first; null unless every value is a parseable model reference. */
export function parseRoles(value: ConfigValue): RoleRow[] | null {
  if (value.kind !== 'map') {
    return null;
  }

  const rows: RoleRow[] = [];

  for (const field of value.value) {
    if (field.value.kind !== 'text') {
      return null;
    }

    const ref = parseModelRef(field.value.value);
    if (ref === null) {
      return null;
    }

    rows.push({ role: field.key, ref, text: field.value.value });
  }

  return [...rows.filter((row) => row.role === 'default'), ...rows.filter((row) => row.role !== 'default')];
}

export type GroupBody =
  | { kind: 'roles'; rows: RoleRow[] }
  /** Settings rows for scalars, then the existing tree for everything with structure. */
  | { kind: 'rows'; scalars: ConfigField[]; nested: ConfigField[] };

export interface Group {
  id: GroupId;
  body: GroupBody;
  /** Entries on show, after any filter. */
  count: number;
}

/** Whether a field is shown as a settings row; containers with entries and long text are left to the tree. */
export function isScalar(value: ConfigValue): boolean {
  return childrenOf(value).length === 0 && !isLongText(value);
}

function rowsBody(fields: ConfigField[], needle: string): GroupBody {
  const shown = needle === '' ? fields : fields.filter((field) => matchesDeep(field.key, field.value, needle));

  return {
    kind: 'rows',
    scalars: shown.filter((field) => isScalar(field.value)),
    nested: shown.filter((field) => !isScalar(field.value)),
  };
}

function bodyCount(body: GroupBody): number {
  return body.kind === 'roles' ? body.rows.length : body.scalars.length + body.nested.length;
}

function buildGroup(id: GroupId, fields: ConfigField[], needle: string): Group | null {
  let body: GroupBody;

  const roles = id === 'modelRoles' && fields.length === 1 ? parseRoles(fields[0].value) : null;
  if (roles !== null) {
    body = { kind: 'roles', rows: needle === '' ? roles : roles.filter((row) => roleMatches(row, needle)) };
  } else if (id === 'modelRoles' && fields.length === 1 && fields[0].value.kind === 'map') {
    // Roles that are not all model references: the plain tree, with the roles themselves as its rows.
    const shown = childrenOf(fields[0].value).filter((role) => needle === '' || matchesDeep(role.key, role.value, needle));
    body = { kind: 'rows', scalars: [], nested: shown };
  } else {
    body = rowsBody(fields, needle);
  }

  const count = bodyCount(body);

  return count === 0 ? null : { id, body, count };
}

/** Splits top-level fields into cards in the fixed order; a card with nothing to show (or nothing matching the query) is left out. */
export function buildGroups(fields: ConfigField[], query: string): Group[] {
  const needle = normalizeNeedle(query);

  return GROUP_ORDER.flatMap((id) => {
    const members = fields.filter((field) => groupOf(field.key) === id);
    const group = members.length === 0 ? null : buildGroup(id, members, needle);

    return group === null ? [] : [group];
  });
}
