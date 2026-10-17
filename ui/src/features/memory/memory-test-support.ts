import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, vi } from 'vitest';

import type { EcosystemStatus } from '$lib/ipc/bindings/EcosystemStatus';
import type { EcosystemRelease } from '$lib/ipc/bindings/EcosystemRelease';
import type { MemoryScriptsAvailability } from '$lib/ipc/bindings/MemoryScriptsAvailability';
import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { sampleScripts, summaryOf } from '$lib/ipc/mock/memory-scripts-fixtures';
import { detailOf, sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { pageOfNotes } from '$lib/ipc/mock/memory-notes-handlers';
import { pageOfScripts } from '$lib/ipc/mock/memory-scripts-handlers';
import { IpcFailure } from '$lib/ipc/transport';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { noteDetail, noteList } from '$lib/stores/memoryNotes.svelte';
import { scriptDetail, scriptList } from '$lib/stores/memoryScripts.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { startMockApp } from '../../../tests/mock-app';
import MemoryView from './MemoryView.svelte';

export const PROJECT = fixtures.recentProjects[0].path;

/** What GitHub says about the latest Harness Memory release next to the installed one. */
export type MemoryRelease = Pick<EcosystemRelease, 'latestVersion' | 'updateAvailable'>;

export const NEWER_RELEASE: MemoryRelease = { latestVersion: '1.1.0', updateAvailable: true };

/** The installed build is a source build, or already the latest release: nothing newer exists. */
export const NO_NEWER_RELEASE: MemoryRelease = { latestVersion: '1.1.0', updateAvailable: false };

/**
 * A Harness Memory that keeps its scripts in this array and answers the three commands like the real one. A function as `availability` is asked at every read, so a test can change what the probe finds.
 * `combined: false` is an older one, which cannot combine the scope with the kind or the status (a function is asked at every read, so a test can update it).
 */
export function fakeMemory(availability: MemoryScriptsAvailability | (() => MemoryScriptsAvailability) = { kind: 'available' }, initial = sampleScripts(), release: MemoryRelease = NEWER_RELEASE, initialNotes = sampleNotes(), combined: boolean | (() => boolean) = true) {
  let scripts = initial;
  let notes = initialNotes;
  const probe = () => (typeof availability === 'function' ? availability() : availability);
  const combines = () => (typeof combined === 'function' ? combined() : combined);
  const passthrough = ipc.invoke.bind(ipc);

  return vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    if (name === 'memory_scripts_list') {
      const asked = args as CommandArgs<'memory_scripts_list'>;
      const visible = scripts.filter((script) => script.scope === 'global' || asked.project !== null);
      const found = probe();
      const page = found.kind === 'available' ? pageOfScripts(visible.map(summaryOf), asked, combines()) : { total: 0, groups: { needsReview: 0, repo: 0, global: 0 }, scopes: null, review: null, combinedFilters: combines(), scripts: [] };

      return { availability: found, ...page } as CommandResult<K>;
    }

    if (name === 'memory_notes_list') {
      const asked = args as CommandArgs<'memory_notes_list'>;
      const found = probe();
      const page = found.kind === 'available' ? pageOfNotes(notes.filter((note) => note.scope === 'global' || asked.project !== null), asked, combines()) : { total: 0, kinds: [], scopes: null, combinedFilters: combines(), notes: [] };

      return { availability: found, ...page } as CommandResult<K>;
    }

    if (name === 'memory_note_get') {
      const found = notes.find((note) => note.id === (args as CommandArgs<'memory_note_get'>).id);
      if (!found) {
        throw IpcFailure.from({ code: 'notFound', message: 'gone' });
      }

      return detailOf(found, notes) as CommandResult<K>;
    }

    if (name === 'memory_note_delete') {
      notes = notes.filter((note) => note.id !== (args as CommandArgs<'memory_note_delete'>).id);

      return null as CommandResult<K>;
    }

    // Deleting everything takes what the project sees: its repository's and the global ones.
    if (name === 'memory_notes_delete_all') {
      const gone = notes.filter((note) => note.scope === 'global' || (args as CommandArgs<'memory_notes_delete_all'>).project !== undefined);
      notes = notes.filter((note) => !gone.includes(note));

      return { deleted: gone.length, team: gone.filter((note) => note.place === 'team').length } as CommandResult<K>;
    }

    if (name === 'memory_scripts_delete_all') {
      const gone = scripts.filter((script) => script.scope === 'global' || (args as CommandArgs<'memory_scripts_delete_all'>).project !== undefined);
      scripts = scripts.filter((script) => !gone.includes(script));

      return { deleted: gone.length, team: gone.filter((script) => script.place === 'team').length } as CommandResult<K>;
    }

    if (name === 'memory_script_get') {
      const found = scripts.find((script) => script.id === (args as CommandArgs<'memory_script_get'>).id);
      if (!found) {
        throw IpcFailure.from({ code: 'notFound', message: 'gone' });
      }

      return found as CommandResult<K>;
    }

    if (name === 'memory_script_delete') {
      scripts = scripts.filter((script) => script.id !== (args as CommandArgs<'memory_script_delete'>).id);

      return null as CommandResult<K>;
    }

    if (name === 'memory_scripts_trust') {
      const { items } = args as CommandArgs<'memory_scripts_trust'>;
      const reviewed = (script: ScriptDetail) => items.some((item) => item.id === script.id && item.sha256 === script.sha256);
      scripts = scripts.map((script) => (reviewed(script) ? { ...script, trusted: true, approval: 'approved' } : script));

      // Like the real server: a script changed since it was reviewed is refused, the others are approved.
      if (items.some((item) => !scripts.some((script) => script.id === item.id && script.sha256 === item.sha256))) {
        throw IpcFailure.from({ code: 'conflict', message: 'a script changed since it was reviewed' });
      }

      return null as CommandResult<K>;
    }

    // Harness Memory is installed (it answers, if too old), next to whatever the Ecosystem page finds.
    if (name === 'ecosystem_status') {
      const status = (await passthrough(name, args)) as EcosystemStatus;

      return { ...status, items: status.items.map((item) => (item.id === 'memory' ? { ...item, installed: true, version: '1.0.1', updateAvailable: false } : item)) } as CommandResult<K>;
    }

    // The release lookup of the Ecosystem page, with the latest Harness Memory release the test chose.
    if (name === 'ecosystem_releases') {
      const others = (await passthrough(name, args) as EcosystemRelease[]).filter((entry) => entry.id !== 'memory');

      return [...others, { id: 'memory', ...release }] as CommandResult<K>;
    }

    return passthrough(name, args);
  });
}

export function calls(invoke: ReturnType<typeof fakeMemory>, name: string): unknown[] {
  return invoke.mock.calls.filter(([command]) => command === name).map(([, args]) => args);
}

/** The table row of a script, by name. */
export const rowButton = (name: string) => document.querySelector<HTMLElement>(`[data-row="${sampleScripts().find((script) => script.name === name)?.id}"]`);

/** Registers what every test of the Memory view needs: a mock backend with the project open, and a clean view before and after each test. */
export function registerMemoryHooks(): void {
  beforeAll(async () => {
    // happy-dom lays nothing out: give every element a 640px viewport so the virtual list renders a window of rows.
    Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 640 });

    await startMockApp(PROJECT);
  });

  beforeEach(() => {
    viewState.reset('memory');
    viewState.memorySection = 'scripts';
    noteList.sync(null, null);
    noteDetail.sync(null, null);
    scriptList.sync(null, null);
    scriptDetail.sync(null, null);
    ecosystem.reset();
  });

  afterEach(() => {
    nav.go('chat');
    toasts.items = [];
    vi.restoreAllMocks();
  });
}

/** The table row of a note, by id. */
export const noteRow = (id: number) => document.querySelector<HTMLElement>(`[data-row="${id}"]`);

/** Shows the Notes list, which is the one a fresh Memory view opens on. */
export async function showNotes(): Promise<void> {
  viewState.memorySection = 'notes';
  nav.go('memory');
  render(MemoryView);
  await screen.findByRole('grid', { name: 'Notes' });
}

export async function showMemory(): Promise<void> {
  nav.go('memory');
  render(MemoryView);
  await screen.findByRole('grid', { name: 'Scripts' });
}

/** Opens a filter menu by its button and picks one of its choices. */
export async function pickFilter(button: string, choice: RegExp): Promise<void> {
  await fireEvent.click(screen.getByRole('button', { name: button }));
  await fireEvent.click(await screen.findByRole('menuitemradio', { name: choice }));
}

/** What a filter menu offers, each choice with its count as the user reads it; the menu is closed again. */
export async function filterChoices(button: string, menu: string): Promise<string[]> {
  await fireEvent.click(screen.getByRole('button', { name: button }));

  const open = await screen.findByRole('menu', { name: menu });
  const choices = within(open).getAllByRole('menuitemradio').map((item) => item.textContent?.trim() ?? '');

  await fireEvent.keyDown(open, { key: 'Escape' });

  return choices;
}
