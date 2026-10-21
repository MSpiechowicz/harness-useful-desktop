import { projectScopes } from '$lib/stores/project-scopes.svelte';

export interface ChatRepository {
  /** The git root: what the guards name the repository by. */
  root: string;
  /** What the repository is called for people. */
  name: string;
}

/**
 * The repository of a chat working in `folder`: its git root, else the folder itself (the identity the guards use, so a folder that is
 * no git repository has its switch too). A folder whose root is not known yet has none, and the switches stay out. Reads the project
 * stores, so call it where a change should show.
 */
export function repositoryOf(folder: string): ChatRepository | null {
  const root = projectScopes.data?.find((scope) => scope.path === folder)?.root ?? null;

  if (root === null) {
    return null;
  }

  return { root, name: root.split(/[\\/]/).filter(Boolean).at(-1) ?? root };
}
