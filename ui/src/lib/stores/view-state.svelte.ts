import type { Sort } from '$lib/components/table';
import type { HookEvent } from '$lib/ipc/bindings/HookEvent';
import type { HookStatus } from '$lib/ipc/bindings/HookStatus';
import type { NoteKind } from '$lib/ipc/bindings/NoteKind';
import type { ScriptGroup } from '$lib/ipc/bindings/ScriptGroup';
import type { ScriptReview } from '$lib/ipc/bindings/ScriptReview';
import type { RoutineStatus } from '$lib/ipc/bindings/RoutineStatus';
import type { RoutineWhenKind } from '$lib/ipc/bindings/RoutineWhenKind';
import type { RunStatus } from '$lib/ipc/bindings/RunStatus';
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

/** What the "Source" filter of the Hooks view narrows the list to: all hooks, the ones of this computer, or the team's. */
export type HookSource = 'all' | 'mine' | 'team';

/** What the "When" filter of the Routines view narrows the list to: all routines, or the ones with one kind of schedule. */
export type RoutineWhenFilter = 'all' | RoutineWhenKind;

/** What the "Status" filter of the Routines view narrows the list to: all routines, or the ones in one status. */
export type RoutineStatusFilter = 'all' | RoutineStatus;

/** What the "Source" filter of the Routines view narrows the list to: all routines, the ones of this computer, or the team's. */
export type RoutineSource = 'all' | 'mine' | 'team';

/** What the "Status" filter of the run history narrows the runs to: all of them, or the ones that ended one way. */
export type RunStatusFilter = 'all' | RunStatus;

/** The two lists the Routines view switches between. */
export type RoutinesSection = 'routines' | 'runs';

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
  /** `sessionId` was asked for by something else (a routine run opened its session): Sessions waits for it, and says so when it is missing, instead of showing another. */
  sessionAsked = $state(false);
  /** Whose sessions Sessions lists: the project's, or the ones global routines left in their own folder. */
  sessionsFolder = $state<'project' | 'routines'>('project');
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

  /** The hook whose panel is open (its row key), what the Hooks search holds, and the three filters (an event, a status, a source; 'all' for none). */
  hookId = $state<string | null>(null);
  hooksQuery = $state('');
  hooksWhen = $state<HookWhen>('all');
  hooksStatus = $state<HookStatusFilter>('all');
  hooksSource = $state<HookSource>('all');
  /** The column the hooks of each group are sorted by; null for the run order. */
  hooksSort = $state<Sort | null>(null);

  /** Which list of the Routines view is on screen, the routine whose panel is open (its row key), what the search holds and the three filters ('all' for none). */
  routinesSection = $state<RoutinesSection>('routines');
  routineId = $state<string | null>(null);
  routinesQuery = $state('');
  routinesWhen = $state<RoutineWhenFilter>('all');
  routinesStatus = $state<RoutineStatusFilter>('all');
  routinesSource = $state<RoutineSource>('all');
  /** The column the routines of each group are sorted by; null for the order they run in. */
  routinesSort = $state<Sort | null>(null);

  /** The run history: the run whose panel is open, the routine it is narrowed to (null for all), the status filter and the page (from 0). */
  runId = $state<string | null>(null);
  runsRoutine = $state<string | null>(null);
  runsStatus = $state<RunStatusFilter>('all');
  runsPage = $state(0);

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
    this.reset('routines');
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
      this.sessionAsked = false;
      this.sessionsFolder = 'project';
      this.sessionsQuery = '';
    } else if (view === 'hooks') {
      this.hookId = null;
      this.hooksQuery = '';
      this.hooksWhen = 'all';
      this.hooksStatus = 'all';
      this.hooksSource = 'all';
      this.hooksSort = null;
    } else if (view === 'routines') {
      this.routinesSection = 'routines';
      this.routineId = null;
      this.routinesQuery = '';
      this.routinesWhen = 'all';
      this.routinesStatus = 'all';
      this.routinesSource = 'all';
      this.routinesSort = null;
      this.runId = null;
      this.runsRoutine = null;
      this.runsStatus = 'all';
      this.runsPage = 0;
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
