import type { MemoryReposPage } from '$lib/ipc/bindings/MemoryReposPage';
import { ipc } from '$lib/ipc/client';

import { ViewLoader } from './view-loader.svelte';

/** The repositories that have notes, with how many each has, for the Scope filter of the Notes view. */
export const noteRepos = new ViewLoader<MemoryReposPage, string>(() => ipc.invoke('memory_repos_list', { kind: 'notes' }));

/** The same for the scripts. */
export const scriptRepos = new ViewLoader<MemoryReposPage, string>(() => ipc.invoke('memory_repos_list', { kind: 'scripts' }));
