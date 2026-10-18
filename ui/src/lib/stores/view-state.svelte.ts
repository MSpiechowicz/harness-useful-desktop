import type { Sort } from '$lib/components/table';
import type { HookEvent } from '$lib/ipc/bindings/HookEvent';
import type { HookStatus } from '$lib/ipc/bindings/HookStatus';
import type { NoteKind } from '$lib/ipc/bindings/NoteKind';
import type { ScriptGroup } from '$lib/ipc/bindings/ScriptGroup';
import type { ScriptReview } from '$lib/ipc/bindings/ScriptReview';
import type { ScriptScope } from '$lib/ipc/bindings/ScriptScope';

import type { ViewId } from './nav.svelte';

/** How many scroll positions are remembered before the least recently written one is dropped. */
const MAX_SCROLLS = 200;

/** The two lists the Memory view switches between. */
export type MemorySection = 'notes' | 'scripts';

/** What the "When" filter of the Hooks view narrows the list to: all hooks, or the ones of one event. */
export type HookWhen = 'all' | HookEvent;

/** What the "Status" filter of the Hooks view narrows the list to: all hooks, or the ones in one status. */
export type HookStatusFilter = 'all' | HookStatus;

/** The settings sections the palette and the section nav can jump to. */
export type SettingsSectionId = 'appearance' | 'startup' | 'harnesses' | 'memory' | 'projects' | 'updates' | 'privacy' | 'sync' | 'about';

/**
 * What each view had selected when the user left it, so coming back (or jumping in from the
 * command palette) lands where they were. Views are unmounted on every switch; this outlives them.
 */
export class ViewState {
  configSection = $state<string | null>(null);
  /** Filter of the open configuration section; the palette sets it to highlight a key. */
  configQuery = $state('');

  /** The kind of the selected extension (an id alone is not unique across kinds). */
  extensionsTab = $state<string | null>(null);
  extensionsItem = $state<string | null>(null);
  extensionsQuery = $state('');

  sessionId = $state<string | null>(null);
  /** What the Sessions search field in the top bar holds. */
  sessionsQuery = $state('');

  /** Which list of the Memory view is on screen. It stays when the project changes, like the language or the theme. */
  memorySection = $state<MemorySection>('notes');

  /** The note whose panel is open, what the search field holds, the scope and kind filters (null for all), the sort and the page (from 0) of the notes table. */
  noteId = $state<number | null>(null);
  notesQuery = $state('');
  notesScope = $state<ScriptScope | null>(null);
  notesKind = $state<NoteKind | null>(null);
  notesSort = $state<Sort>({ key: 'updated', desc: true });
  notesPage = $state(0);

  /** The same for the scripts table, with its scope and status filters (null for all). `scriptsGroup` is the one "Show" filter of a Harness Memory that cannot combine them. */
  scriptId = $state<number | null>(null);
  scriptsQuery = $state('');
  scriptsScope = $state<ScriptScope | null>(null);
  scriptsStatus = $state<ScriptReview | null>(null);
  scriptsGroup = $state<ScriptGroup | null>(null);
  scriptsSort = $state<Sort>({ key: 'updated', desc: true });
  scriptsPage = $state(0);

  /** The hook whose panel is open, what the Hooks search holds, and the two filters (an event, a status; 'all' for none). */
  hookId = $state<string | null>(null);
  hooksQuery = $state('');
  hooksWhen = $state<HookWhen>('all');
  hooksStatus = $state<HookStatusFilter>('all');

  /** A settings section waiting to be scrolled to; the settings view clears it once it has. */
  settingsTarget = $state<SettingsSectionId | null>(null);

  /** Scroll positions by key, least recently written first; capped because keys are per item. */
  private scrolls = new Map<string, number>();

  scrollOf(key: string): number {
    return this.scrolls.get(key) ?? 0;
  }

  rememberScroll(key: string, top: number): void {
    this.scrolls.delete(key);
    this.scrolls.set(key, top);

    if (this.scrolls.size > MAX_SCROLLS) {
      const oldest = this.scrolls.keys().next();

      if (!oldest.done) {
        this.scrolls.delete(oldest.value);
      }
    }
  }

  /** Forgets all remembered state: a fresh start. */
  resetAll(): void {
    this.reset('config');
    this.reset('extensions');
    this.reset('sessions');
    this.reset('memory');
    this.reset('hooks');
    this.settingsTarget = null;
    this.scrolls.clear();
  }

  /** Forgets everything for one view (e.g. after switching harness, when ids no longer apply). */
  reset(view: ViewId): void {
    if (view === 'config') {
      this.configSection = null;
      this.configQuery = '';
    } else if (view === 'extensions') {
      this.extensionsTab = null;
      this.extensionsItem = null;
      this.extensionsQuery = '';
    } else if (view === 'sessions') {
      this.sessionId = null;
      this.sessionsQuery = '';
    } else if (view === 'hooks') {
      this.hookId = null;
      this.hooksQuery = '';
      this.hooksWhen = 'all';
      this.hooksStatus = 'all';
    } else if (view === 'memory') {
      this.noteId = null;
      this.notesQuery = '';
      this.notesScope = null;
      this.notesKind = null;
      this.notesSort = { key: 'updated', desc: true };
      this.notesPage = 0;
      this.scriptId = null;
      this.scriptsQuery = '';
      this.scriptsScope = null;
      this.scriptsStatus = null;
      this.scriptsGroup = null;
      this.scriptsSort = { key: 'updated', desc: true };
      this.scriptsPage = 0;
    }
  }
}

export const viewState = new ViewState();
