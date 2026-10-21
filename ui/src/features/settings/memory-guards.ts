import type { MessageKey } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
import type { GuardKind } from '$lib/stores/memory-chat';

/** The words of one kind's card, as message keys. */
export const GUARD_TEXT = {
  notes: {
    title: 'memory.guards.settings.notes',
    pause: 'memory.guards.settings.pause.notes',
    paused: 'memory.guards.settings.paused.notes',
    ask: 'memory.guards.settings.ask.notes',
    repos: 'memory.guards.settings.repos.notes',
    reposHelp: 'memory.guards.settings.repos.help.notes',
    none: 'memory.guards.settings.repos.none.notes',
    removeTitle: 'memory.guards.settings.repos.removeTitle.notes',
    removeBody: 'memory.guards.settings.repos.removeBody.notes',
  },
  scripts: {
    title: 'memory.guards.settings.scripts',
    pause: 'memory.guards.settings.pause.scripts',
    paused: 'memory.guards.settings.paused.scripts',
    ask: 'memory.guards.settings.ask.scripts',
    repos: 'memory.guards.settings.repos.scripts',
    reposHelp: 'memory.guards.settings.repos.help.scripts',
    none: 'memory.guards.settings.repos.none.scripts',
    removeTitle: 'memory.guards.settings.repos.removeTitle.scripts',
    removeBody: 'memory.guards.settings.repos.removeBody.scripts',
  },
} as const satisfies Record<GuardKind, Record<string, MessageKey>>;

/** The preference holding the "Ask before saving" a new chat starts with. */
export const ASK_PREF = { notes: 'memoryNotesAskDefault', scripts: 'memoryScriptsAskDefault' } as const satisfies Record<GuardKind, string>;

/** Lets the user pick a repository folder in the native dialog. It changes no project. Null when the dialog was cancelled. */
export function pickRepositoryFolder(): Promise<string | null> {
  return ipc.invoke('dialog_pick_folder', undefined);
}

/** The last folder of a path: what a repository is called in a list. */
export function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}
