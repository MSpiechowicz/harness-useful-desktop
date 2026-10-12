import { t, tn } from '$lib/i18n/i18n.svelte';
import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import type { ConfigValue } from '$lib/ipc/bindings/ConfigValue';
import type { SecretKind } from '$lib/ipc/bindings/SecretKind';

/** One visible line of the flattened tree. */
export interface TreeRow {
  id: string;
  parentId: string | null;
  depth: number;
  key: string;
  value: ConfigValue;
  /** Maps and lists with at least one child. */
  branch: boolean;
  /** Long text that is clipped until expanded. */
  longText: boolean;
  /** For branches: children shown. For long text: full text shown. */
  expanded: boolean;
  childCount: number;
  posInSet: number;
  setSize: number;
}

export interface FlattenOptions {
  /** Case-insensitive filter; empty shows everything. */
  query: string;
  /** User toggles by row id; absent ids use the defaults. */
  overrides: ReadonlyMap<string, boolean>;
  /** Branches shallower than this start expanded when not filtering. */
  defaultDepth: number;
}

const LONG_TEXT_CHARS = 96;

export function isLongText(value: ConfigValue): boolean {
  return value.kind === 'text' && (value.value.length > LONG_TEXT_CHARS || value.value.includes('\n'));
}

/** Child entries of a map (by key) or list (by `[index]`); empty for scalars and secrets. */
export function childrenOf(value: ConfigValue): ConfigField[] {
  if (value.kind === 'map') {
    return value.value;
  }
  if (value.kind === 'list') {
    return value.value.map((item, index) => ({ key: `[${index}]`, value: item }));
  }

  return [];
}

/** The plain text of a non-secret scalar; secrets and containers have none, so they can never be matched. */
export function scalarText(value: ConfigValue): string | null {
  switch (value.kind) {
    case 'text':
    case 'number':
      return value.value;
    case 'bool':
      return String(value.value);
    case 'null':
      return 'null';
    default:
      return null;
  }
}

export function secretLabel(secret: SecretKind): string {
  switch (secret.type) {
    case 'value':
      return t('components.config.secret.value');
    case 'command':
      return t('components.config.secret.command');
    case 'envRef':
      return `$${secret.name}`;
  }
}

export function secretHint(secret: SecretKind): string {
  switch (secret.type) {
    case 'value':
      return t('components.config.hint.value');
    case 'command':
      return t('components.config.hint.command');
    case 'envRef':
      return t('components.config.hint.envRef', { name: secret.name });
  }
}

/** "3 keys", "1 item", "empty" for a container; null for scalars. */
export function describeContainer(value: ConfigValue): string | null {
  if (value.kind !== 'map' && value.kind !== 'list') {
    return null;
  }

  const count = value.value.length;

  if (count === 0) {
    return t('components.config.empty');
  }

  return value.kind === 'map' ? tn('components.config.key', count) : tn('components.config.item', count);
}

function matchesSelf(key: string, value: ConfigValue, needle: string): boolean {
  if (key.toLowerCase().includes(needle)) {
    return true;
  }

  return scalarText(value)?.toLowerCase().includes(needle) ?? false;
}

function matchesDeep(key: string, value: ConfigValue, needle: string): boolean {
  return matchesSelf(key, value, needle) || childrenOf(value).some((child) => matchesDeep(child.key, child.value, needle));
}

function hasMatchingDescendant(value: ConfigValue, needle: string): boolean {
  return childrenOf(value).some((child) => matchesDeep(child.key, child.value, needle));
}

/** Number of entries (at any depth) whose key or value matches the query. */
export function countMatches(fields: ConfigField[], query: string): number {
  const needle = query.trim().toLowerCase();
  if (needle === '') {
    return 0;
  }

  return fields.reduce((sum, field) => {
    const own = matchesSelf(field.key, field.value, needle) ? 1 : 0;

    return sum + own + countMatches(childrenOf(field.value), needle);
  }, 0);
}

/**
 * Flattens the tree to the rows currently visible. With a query, only entries that match,
 * lead to a match, or sit below a matching entry remain, and their ancestors open up.
 */
export function flattenTree(fields: ConfigField[], options: FlattenOptions): TreeRow[] {
  const needle = options.query.trim().toLowerCase();
  const rows: TreeRow[] = [];

  function walk(entries: ConfigField[], depth: number, parentId: string | null, underMatch: boolean): void {
    const visible = entries.filter((entry) => needle === '' || underMatch || matchesDeep(entry.key, entry.value, needle));

    visible.forEach((entry, index) => {
      const id = parentId === null ? entry.key : `${parentId}\u001f${entry.key}`;
      const children = childrenOf(entry.value);
      const branch = children.length > 0;
      const longText = isLongText(entry.value);
      const selfMatch = needle !== '' && matchesSelf(entry.key, entry.value, needle);

      const openByDefault = needle === '' ? depth < options.defaultDepth : selfMatch || hasMatchingDescendant(entry.value, needle);
      const expanded = options.overrides.get(id) ?? (branch ? openByDefault : false);

      rows.push({
        id,
        parentId,
        depth,
        key: entry.key,
        value: entry.value,
        branch,
        longText,
        expanded,
        childCount: children.length,
        posInSet: index + 1,
        setSize: visible.length,
      });

      if (branch && expanded) {
        walk(children, depth + 1, id, underMatch || selfMatch);
      }
    });
  }

  walk(fields, 0, null, false);

  return rows;
}

/** Ids of every row that can open (branches and long texts) at any depth, for collapse all / expand all. */
export function toggleableIds(fields: ConfigField[], parentId: string | null = null): string[] {
  return fields.flatMap((entry) => {
    const id = parentId === null ? entry.key : `${parentId}\u001f${entry.key}`;
    const children = childrenOf(entry.value);
    const own = children.length > 0 || isLongText(entry.value) ? [id] : [];

    return [...own, ...toggleableIds(children, id)];
  });
}
