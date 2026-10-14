import { childrenOf } from '$lib/components/config-tree';
import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import type { ConfigValue } from '$lib/ipc/bindings/ConfigValue';

export interface ConfigStats {
  /** Entries at the top level of the file. */
  entries: number;
  /** Values that hold no other values: scalars, secrets and empty maps or lists. */
  values: number;
  /** Levels of nesting, counting the top level: a flat file is 1. */
  depth: number;
  /** Values the backend masked. */
  secrets: number;
}

interface Measure {
  values: number;
  /** Levels below this value: 0 for a leaf. */
  depth: number;
  secrets: number;
}

function measure(value: ConfigValue): Measure {
  const children = childrenOf(value);

  if (children.length === 0) {
    return { values: 1, depth: 0, secrets: value.kind === 'secret' ? 1 : 0 };
  }

  const inner = children.map((child) => measure(child.value));

  return {
    values: inner.reduce((sum, part) => sum + part.values, 0),
    depth: 1 + Math.max(...inner.map((part) => part.depth)),
    secrets: inner.reduce((sum, part) => sum + part.secrets, 0),
  };
}

/** The numbers of the Overview card; an empty file has none of anything. */
export function configStats(fields: ConfigField[]): ConfigStats {
  const parts = fields.map((field) => measure(field.value));

  return {
    entries: fields.length,
    values: parts.reduce((sum, part) => sum + part.values, 0),
    depth: parts.length === 0 ? 0 : 1 + Math.max(...parts.map((part) => part.depth)),
    secrets: parts.reduce((sum, part) => sum + part.secrets, 0),
  };
}
