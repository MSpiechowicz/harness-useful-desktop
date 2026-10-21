import { ipc } from '$lib/ipc/client';
import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

import { project } from './project.svelte';
import { ViewLoader } from './view-loader.svelte';

/** The repositories the user works with on this computer (the open project and the recent ones), each with the key Harness Memory knows it by, for the Scope filters and the editors' Applies to field. */
export const projectScopes = new ViewLoader<ProjectScope[], string>(() => ipc.invoke('project_scopes', undefined));

/** The loader has one answer at a time, so its pair is a fixed key and everything that changes the answer: the open project and the recent ones. */
const KEY = 'scopes';

function stamp(): string {
  return [project.active?.path ?? '', ...project.recents.map((recent) => recent.path)].join('\u0000');
}

/** Makes the projects match the open project and the recent ones, and quietly re-reads them when a view opens, so a key that became known meanwhile shows. */
export function revalidateProjectScopes(): void {
  projectScopes.revalidate(KEY, stamp());
}
