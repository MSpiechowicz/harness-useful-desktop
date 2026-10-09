export type ClassPart = string | false | null | undefined;

/** Joins truthy class fragments; the single place components compose class strings. */
export function cx(...parts: ClassPart[]): string {
  return parts.filter(Boolean).join(' ');
}
