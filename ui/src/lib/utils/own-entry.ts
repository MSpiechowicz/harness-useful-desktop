/**
 * A table's own entry for a name read from outside the app. A plain `table[name]` would find what every object inherits
 * for names such as "constructor".
 */
export function ownEntry<T>(table: Record<string, T>, name: string): T | undefined {
  return Object.hasOwn(table, name) ? table[name] : undefined;
}
