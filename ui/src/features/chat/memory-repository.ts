import { memoryChat } from '$lib/stores/memoryChat.svelte';

export interface ChatRepository {
  /** The saving root: what the guards list a project by (a git worktree counts as its main repository). */
  root: string;
  /** What the chat's project is called for people: the name of the folder the chat works in. */
  name: string;
}

/**
 * The project of a chat working in `folder`: the saving root the global guards list it under, named after the folder itself. A folder
 * whose root is not known yet (or that is gone) has none, so it counts as not turned on and the project switches stay out. Reads
 * the root cache, so call it where a change should show.
 */
export function repositoryOf(folder: string): ChatRepository | null {
  const root = memoryChat.rootOf(folder);

  if (root === null) {
    return null;
  }

  return { root, name: folder.split(/[\\/]/).filter(Boolean).at(-1) ?? folder };
}
