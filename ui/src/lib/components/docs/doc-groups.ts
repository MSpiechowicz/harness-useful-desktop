/** How many documents (hooks, routines) one scope, the global ones or one project's, can hold. */
export const MAX_PER_SCOPE = 64;

/** Whether a scope already holds the most documents it can: a document that stays in its scope (`stays`) is not counted twice. */
export function scopeIsFull(held: number, stays: boolean): boolean {
  return !stays && held >= MAX_PER_SCOPE;
}

/** One block of a documents list: the global ones, the project's, the files that are not documents (invalid), or the team's. */
export interface DocGroup<V> {
  id: 'global' | 'project' | 'invalid' | 'team';
  views: V[];
  /** How many documents the group holds in all, whatever the search or the filter shows. */
  total: number;
}

/**
 * The documents that answer the search and the filter, in their groups. A group the search or filter leaves empty is left out,
 * while the unfiltered list always shows both scopes, so an empty scope can say so and offer a first document. The team's group
 * comes last and shows (empty) only while documents are shared with the team (`teamOn`).
 */
export function groupDocs<V>(all: V[], shown: V[], narrowed: boolean, teamOn: boolean, groupOf: (view: V) => DocGroup<V>['id']): DocGroup<V>[] {
  const groups = (['global', 'project', 'invalid', 'team'] as const).map((id) => ({
    id,
    views: shown.filter((view) => groupOf(view) === id),
    total: all.filter((view) => groupOf(view) === id).length,
  }));

  return groups.filter((group) => {
    if (group.views.length > 0) {
      return true;
    }

    return group.id === 'invalid' || narrowed ? false : group.id !== 'team' || teamOn;
  });
}

/** A column of a documents table: the classes of its `<col>` (width, when it shows) and of its header cell. */
export interface DocColumn {
  label: string;
  col?: string;
  head?: string;
  /** The header's text lines up on the right, over a column of switches. */
  right?: boolean;
}

/** What a documents table lets its view do: put the focus on a row, e.g. back on the one whose panel was just closed. */
export interface DocTableHandle {
  focusRow: (id: string) => void;
}

/** What the buttons of a document's panel can ask for. */
export interface DocActionHandlers {
  onedit: () => void;
  ondelete: () => void;
  onshare: () => void;
  onupdate: () => void;
  onremove: () => void;
  oncopy: () => void;
}

/** What a list of documents lets its view do. */
export interface DocListHandlers<V> {
  onopen: (id: string) => void;
  /** Moves the open panel along when the arrow keys move the focus. */
  onmove: (id: string) => void;
  ontoggle: (view: V, enabled: boolean) => void;
  onadd: (scope: 'global' | 'project') => void;
  /** The table, for the view to put the focus back on a row once its panel is closed. */
  table?: DocTableHandle;
}
