import type { AuthorFacet } from '$lib/ipc/bindings/AuthorFacet';
import type { AuthorFilter } from '$lib/ipc/bindings/AuthorFilter';

/** The menu value of "no author filter". */
export const ANY_AUTHOR = 'all';

/** The menu value of the entries nobody is known to have added. */
export const UNKNOWN_AUTHOR = 'unknown';

/** A person's menu value starts with this, so a person called "all" or "unknown" is never mistaken for those choices. */
const NAME_PREFIX = 'name:';

/** The menu value of an author filter: `all`, `unknown` or `name:<name>`. */
export function authorValue(filter: AuthorFilter | null | undefined): string {
  if (!filter) {
    return ANY_AUTHOR;
  }

  return filter.kind === 'unknown' ? UNKNOWN_AUTHOR : `${NAME_PREFIX}${filter.name}`;
}

/** The author filter a menu value stands for; null for `all` and for a value that is none of the three forms. */
export function authorFromValue(value: string): AuthorFilter | null {
  if (value === UNKNOWN_AUTHOR) {
    return { kind: 'unknown' };
  }

  return value.startsWith(NAME_PREFIX) ? { kind: 'name', name: value.slice(NAME_PREFIX.length) } : null;
}

/** The most people the servers list in a facet: a facet with that many may have left some out. */
export const AUTHOR_FACET_CAP = 200;

/** One choice of the Added by menu. A count is shown only when the list says it. */
export interface AuthorOption {
  value: string;
  label: string;
  count?: number;
}

/** The words of the choices that are not people. */
export interface AuthorLabels {
  all: string;
  unknown: string;
}

/**
 * The choices of the Added by menu: All, each person the list names, and Unknown while some entries have no known author (or it is the chosen one).
 * The chosen author is always among them, so the button never names a choice the menu lacks. When the other filters leave nothing of them it has a zero count,
 * unless the facet is full (`AUTHOR_FACET_CAP`), which may have left them out: their count is then not known.
 * The All count is the sum of the facet, so it is left out whenever that cannot be right: a full facet, or a chosen person the facet does not name.
 */
export function authorOptions(facet: AuthorFacet | null, chosen: AuthorFilter | null, labels: AuthorLabels): AuthorOption[] {
  const people: AuthorOption[] = (facet?.people ?? []).map((person) => ({ value: authorValue({ kind: 'name', name: person.name }), label: person.name, count: person.count }));

  const full = (facet?.people.length ?? 0) >= AUTHOR_FACET_CAP;
  const missing = chosen?.kind === 'name' && !people.some((person) => person.value === authorValue(chosen));

  if (chosen?.kind === 'name' && missing) {
    people.push({ value: authorValue(chosen), label: chosen.name, count: facet && !full ? 0 : undefined });
  }

  const unknown = facet?.unknown ?? 0;
  const everyone = facet && !full && !missing ? people.reduce((sum, person) => sum + (person.count ?? 0), unknown) : undefined;
  const options: AuthorOption[] = [{ value: ANY_AUTHOR, label: labels.all, count: everyone }, ...people];

  if (unknown > 0 || chosen?.kind === 'unknown') {
    options.push({ value: UNKNOWN_AUTHOR, label: labels.unknown, count: facet ? unknown : undefined });
  }

  return options;
}

/**
 * Whether a list shows the Added by menu: the server filters by author, and there is somebody to choose, or the list is narrowed some way (a search, a filter,
 * this one too) so that the toolbar does not change when another filter empties the choices. A list of only unknown authors, or of nothing, has none.
 */
export function authorMenuShown(supported: boolean, facet: AuthorFacet | null, narrowed: boolean): boolean {
  return supported && ((facet?.people.length ?? 0) > 0 || narrowed);
}

/** The choices from which the menu gets a search field. */
export const AUTHOR_SEARCH_FROM = 8;
