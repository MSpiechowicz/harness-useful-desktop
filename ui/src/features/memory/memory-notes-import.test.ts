import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import type { NotesImportPreview } from '$lib/ipc/bindings/NotesImportPreview';
import { IpcFailure } from '$lib/ipc/transport';
import { nav } from '$lib/stores/nav.svelte';
import { project } from '$lib/stores/project.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { appliesTo, calls, EMPTY_PROJECT, fakeMemory, IMPORT_TAKEN_TITLE, importRow, pickTarget, PROJECT, registerMemoryHooks, SAMPLE_IMPORT, showNotes } from './memory-test-support';
import { notesImport } from './notes-import-state.svelte';
import NotesView from './NotesView.svelte';

registerMemoryHooks();

afterEach(() => notesImport.reset());

const NO_NOTES = { kind: 'available' } as const;

/** Chooses "Import from file…" in the "⋯" menu of the Notes header. */
async function chooseImport(): Promise<void> {
  await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
  await fireEvent.click(await screen.findByRole('menuitem', { name: 'Import from file…' }));
}

/** Opens the dialog on the format a file needs, without picking a file. */
async function openStart(): Promise<HTMLElement> {
  await showNotes();
  await chooseImport();

  return screen.findByRole('dialog', { name: 'Import notes' });
}

/** Picks the file of `fakeMemory` and waits for the review of it. */
async function openReview(): Promise<HTMLElement> {
  const dialog = await openStart();

  await fireEvent.click(within(dialog).getByRole('button', { name: 'Choose file…' }));
  await waitFor(() => expect(notesImport.stage).not.toBe('start'));

  return dialog;
}

/** The checkbox of one note of the file, by its title. */
const tickOf = (dialog: HTMLElement, title: string) => within(dialog).getAllByRole('checkbox', { name: `Import “${title}”` });

/** A file of `count` notes, for the paging of the review. */
function bigFile(count: number): NotesImportPreview {
  const rows = Array.from({ length: count }, (_, index) => importRow(index, `Note ${index + 1}`));

  return { kind: 'plan', plan: { planId: 'plan-big', fileName: 'many.json', format: 'json', truncated: true, rows } };
}

describe('the format of a file', () => {
  it('opens on the fields of a note and an example, and does not pick a file until asked to', async () => {
    const invoke = fakeMemory();
    const dialog = await openStart();

    expect(within(dialog).getByText('Notes come from a CSV or JSON file with one note per row. Check the format, then choose the file.')).toBeInTheDocument();
    for (const [field, need, rule] of [
      ['title', 'Required', 'One line, up to 120 characters.'],
      ['body', 'Required', 'Up to 4000 characters.'],
      ['kind', 'Optional', 'Empty means note.'],
      ['tags', 'Optional', /Up to 8, separated by commas or spaces/],
      ['scope', 'Optional', 'Empty uses the default chosen at review.'],
    ] as const) {
      const row = within(dialog).getByText(field, { selector: 'dt' }).closest('div') as HTMLElement;
      expect(row).toHaveTextContent(need);
      expect(row).toHaveTextContent(rule);
    }

    const kinds = within(dialog).getByText('kind', { selector: 'dt' }).closest('div') as HTMLElement;
    expect(within(kinds).getAllByRole('listitem').map((item) => item.textContent?.replace('/', '').trim())).toEqual(['outcome', 'problem', 'fix', 'optimisation', 'location', 'decision', 'note']);

    expect(within(dialog).getByText(/title,body,kind,tags,scope/)).toBeInTheDocument();
    expect(within(dialog).getByText('A file holds up to 500 notes and is at most 2 MiB.')).toBeInTheDocument();
    expect(calls(invoke, 'memory_notes_import_preview')).toEqual([]);
  });

  it('shows the example in the format that is chosen', async () => {
    fakeMemory();
    const dialog = await openStart();

    await fireEvent.click(within(dialog).getByRole('radio', { name: 'JSON file' }));

    expect(within(dialog).getByText(/"notes": \[/)).toBeInTheDocument();
    expect(within(dialog).queryByText(/title,body,kind,tags,scope/)).toBeNull();
  });

  it('runs the picker with the button, and reviews the file it hands over', async () => {
    const invoke = fakeMemory();
    const dialog = await openStart();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Choose file…' }));

    expect(await within(dialog).findByText('team-notes.csv · CSV file · 5 notes')).toBeInTheDocument();
    expect(calls(invoke, 'memory_notes_import_preview')).toEqual([{ project: PROJECT }]);
  });

  it('stays on the format when the picker is closed without a file', async () => {
    const invoke = fakeMemory(undefined, undefined, undefined, undefined, undefined, { preview: null });
    const dialog = await openStart();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Choose file…' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_import_preview')).toHaveLength(1));
    expect(within(dialog).getByRole('button', { name: 'Choose file…' })).toBeEnabled();
    expect(within(dialog).getByText('Up to 4000 characters.')).toBeInTheDocument();
  });

  it('saves an example file in the format that is chosen and says where it went', async () => {
    const invoke = fakeMemory();
    const dialog = await openStart();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save example file' }));
    await waitFor(() => expect(toasts.items.map((toast) => [toast.title, toast.description, toast.tone])).toContainEqual(['Example file saved', '/home/demo/notes-example.csv', 'success']));

    await fireEvent.click(within(dialog).getByRole('radio', { name: 'JSON file' }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save example file' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_import_template')).toEqual([{ format: 'csv' }, { format: 'json' }]));
  });

  it('says nothing when the save dialog is closed, and warns when the file could not be written', async () => {
    const invoke = fakeMemory();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;
    let answer: string | null | Error = null;
    invoke.mockImplementation(((name: CommandName, args: never) => {
      if (name !== 'memory_notes_import_template') {
        return real(name as never, args);
      }

      return answer instanceof Error ? Promise.reject(answer) : Promise.resolve(answer);
    }) as typeof ipc.invoke);
    const dialog = await openStart();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save example file' }));
    await waitFor(() => expect(calls(invoke, 'memory_notes_import_template')).toHaveLength(1));
    expect(toasts.items).toEqual([]);

    answer = IpcFailure.from({ code: 'io', message: 'disk is full' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save example file' }));

    await waitFor(() => expect(toasts.items.map((toast) => [toast.title, toast.description, toast.tone])).toEqual([['The example file was not saved', 'disk is full', 'warning']]));
  });
});

describe('reviewing a file', () => {
  it('lists the notes of the file, with problems disabled and repeats unticked', async () => {
    const invoke = fakeMemory();
    const dialog = await openReview();

    expect(calls(invoke, 'memory_notes_import_preview')).toEqual([{ project: PROJECT }]);
    expect(within(dialog).getByText('team-notes.csv · CSV file · 5 notes')).toBeInTheDocument();

    // The note that is new, the one that a repository note has the title of and the global one are ticked. The one with a problem cannot be, the repeat is not.
    expect(tickOf(dialog, 'Retry a flaky test once')[0]).toBeChecked();
    expect(tickOf(dialog, IMPORT_TAKEN_TITLE)[0]).toBeChecked();
    expect(tickOf(dialog, 'Prefer the release profile for benchmarks')[0]).toBeChecked();
    expect(within(dialog).getByRole('checkbox', { name: 'Import note 3' })).toBeDisabled();
    expect(tickOf(dialog, 'Retry a flaky test once')[1]).not.toBeChecked();
    expect(tickOf(dialog, 'Retry a flaky test once')[1]).toBeEnabled();

    expect(within(dialog).getByText('Title: missing')).toBeInTheDocument();
    expect(within(dialog).getByText('Same title as note 1')).toBeInTheDocument();
    expect(within(dialog).getByText('3 of 4 selected')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Import 3 notes' })).toBeEnabled();
  });

  it('selects all the notes that can be imported, or none', async () => {
    fakeMemory();
    const dialog = await openReview();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Select all' }));
    expect(within(dialog).getByText('4 of 4 selected')).toBeInTheDocument();
    expect(within(dialog).getByRole('checkbox', { name: 'Import note 3' })).not.toBeChecked();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Select none' }));
    expect(within(dialog).getByText('0 of 4 selected')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Import 0 notes' })).toBeDisabled();
  });

  it('pages a file of many notes and says that only the first were read', async () => {
    fakeMemory(undefined, undefined, undefined, undefined, undefined, { preview: bigFile(120) });
    const dialog = await openReview();

    expect(within(dialog).getByText(/Only the first 500 are shown/)).toBeInTheDocument();
    expect(within(dialog).getByText('Note 50')).toBeInTheDocument();
    expect(within(dialog).queryByText('Note 51')).toBeNull();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Next' }));
    expect(within(dialog).getByText('Note 51')).toBeInTheDocument();
    expect(within(dialog).queryByText('Note 50')).toBeNull();
    expect(within(dialog).getByText('120 of 120 selected')).toBeInTheDocument();
  });

  it('keeps the notes without a scope in the open project, or global', async () => {
    fakeMemory();
    const dialog = await openReview();

    expect(within(dialog).getByText('Applies to')).toBeInTheDocument();
    expect(appliesTo(dialog)).toHaveTextContent('demo');
    expect(within(dialog).getByText('Notes whose file gives no scope are kept in demo.')).toBeInTheDocument();

    // The file was read for the open project, so that and Global are all it can offer.
    await fireEvent.click(appliesTo(dialog));
    const menu = await screen.findByRole('menu', { name: 'Applies to' });
    expect(within(menu).getAllByRole('menuitemradio').map((item) => item.textContent?.replace(/\s+/g, ' ').trim())).toEqual(['Global', 'demo useful/harness-desktop']);
  });

  it('reads the file for the project the Scope filter looks at', async () => {
    viewState.memoryReach = 'repo:path:' + EMPTY_PROJECT;
    const invoke = fakeMemory();
    // The project has no notes, so there is no table to wait for.
    nav.go('notes');
    render(NotesView);
    await fireEvent.click(await screen.findByRole('button', { name: 'More actions' }));
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Import from file…' }));
    const dialog = await screen.findByRole('dialog', { name: 'Import notes' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Choose file…' }));
    await waitFor(() => expect(notesImport.stage).toBe('review'));

    expect(calls(invoke, 'memory_notes_import_preview')).toEqual([{ project: EMPTY_PROJECT }]);
    expect(appliesTo(dialog)).toHaveTextContent('research');

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Import 3 notes' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_import_apply')).toMatchObject([{ defaultScope: 'repo', project: EMPTY_PROJECT }]));
  });

  it('reads the file for no project when the Scope filter looks at the global notes', async () => {
    viewState.memoryReach = 'global';
    const invoke = fakeMemory();
    const dialog = await openReview();

    expect(calls(invoke, 'memory_notes_import_preview')).toEqual([{ project: null }]);
    expect(appliesTo(dialog)).toHaveTextContent('Global');
    expect(within(dialog).getByText(/Open a project to keep them in its project/)).toBeInTheDocument();
  });
});

describe('the problems of a row', () => {
  it('says that a tag has characters other than letters, digits, - and _', async () => {
    const preview: NotesImportPreview = { kind: 'plan', plan: { planId: 'plan-tags', fileName: 'tags.csv', format: 'csv', truncated: false, rows: [importRow(0, 'Tagged note', { tags: ['ci!'], problems: [{ code: 'invalidChars', field: 'tags' }] })] } };
    fakeMemory(undefined, undefined, undefined, undefined, undefined, { preview });
    const dialog = await openReview();

    expect(within(dialog).getByText('Tags: use only a–z, 0–9, - and _')).toBeInTheDocument();
    expect(tickOf(dialog, 'Tagged note')[0]).toBeDisabled();
  });
});

describe('importing the notes', () => {
  it('sends the ticked notes with the scope and the replace choice, shows the progress, then what was done', async () => {
    let finish = () => {};
    const hold = new Promise<void>((resolve) => (finish = resolve));
    const invoke = fakeMemory(undefined, undefined, undefined, undefined, undefined, { preview: SAMPLE_IMPORT, hold });
    const dialog = await openReview();

    await pickTarget(dialog, 'Global');
    await fireEvent.click(tickOf(dialog, IMPORT_TAKEN_TITLE)[0]);
    await fireEvent.click(tickOf(dialog, 'Retry a flaky test once')[1]);
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Import 3 notes' }));

    // The first of the three is saved and the run waits: the bar says so, and the dialog cannot be left.
    const bar = await screen.findByRole('progressbar', { name: 'Import progress' });
    await waitFor(() => expect(bar).toHaveAttribute('aria-valuenow', '1'));
    expect(bar).toHaveAttribute('aria-valuemax', '3');
    expect(screen.getByText('1 of 3 notes')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Close' })).toBeNull();
    expect(calls(invoke, 'memory_notes_import_apply')).toMatchObject([{ planId: 'plan-1', rows: [0, 3, 4], defaultScope: 'global', replace: false, project: PROJECT }]);

    finish();

    expect(await screen.findByText('3 notes were written to Harness Memory.')).toBeInTheDocument();
    const counts = within(screen.getByRole('dialog', { name: 'Import finished' }));
    expect(counts.getByText('Created').nextElementSibling).toHaveTextContent('2');
    // The repeat of the first title is global too, which the server rewrites.
    expect(counts.getByText('Updated').nextElementSibling).toHaveTextContent('1');
    expect(counts.getByText('Skipped').nextElementSibling).toHaveTextContent('0');
  });

  it('lists the notes that were skipped for a title that is taken, and opens the one that has it', async () => {
    fakeMemory();
    const dialog = await openReview();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Import 3 notes' }));

    const skipped = await screen.findByRole('region', { name: 'Skipped notes' });
    expect(within(skipped).getByText(IMPORT_TAKEN_TITLE)).toBeInTheDocument();

    await fireEvent.click(within(skipped).getByRole('button', { name: `Open “${IMPORT_TAKEN_TITLE}”` }));

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(viewState.noteId).toBe(3);
  });

  it('replaces the repository notes with the same title when asked to, and shows the new notes in the list', async () => {
    const invoke = fakeMemory();
    const dialog = await openReview();

    await fireEvent.click(within(dialog).getByRole('switch', { name: 'Replace notes with the same title' }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Import 3 notes' }));

    await screen.findByText('3 notes were written to Harness Memory.');
    expect(calls(invoke, 'memory_notes_import_apply')).toMatchObject([{ replace: true }]);
    expect(screen.queryByRole('region', { name: 'Skipped notes' })).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(await screen.findByText('Retry a flaky test once')).toBeInTheDocument();
  });

  it('says why the import could not run and offers another file', async () => {
    const invoke = fakeMemory();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;
    invoke.mockImplementation(((name: CommandName, args: never) =>
      name === 'memory_notes_import_apply' ? Promise.reject(IpcFailure.from({ code: 'unsupported', message: 'update Harness Memory to save notes' })) : real(name as never, args)) as typeof ipc.invoke);
    const dialog = await openReview();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Import 3 notes' }));

    const failed = await screen.findByRole('dialog', { name: 'Import failed' });
    expect(within(failed).getByRole('alert')).toHaveTextContent('update Harness Memory to save notes');
    expect(within(failed).getByRole('button', { name: 'Choose another file' })).toBeEnabled();
    expect(within(failed).getByRole('button', { name: 'Done' })).toBeEnabled();
  });
});

describe('a file that cannot be used', () => {
  const refused = (reason: Extract<NotesImportPreview, { kind: 'refused' }>['reason'], line: number | null = null): NotesImportPreview => ({ kind: 'refused', fileName: 'notes.csv', reason, line });

  it('says why in the dialog and lets the user choose another file', async () => {
    fakeMemory(undefined, undefined, undefined, undefined, undefined, { preview: refused('missingColumns') });
    const dialog = await openReview();

    expect(within(dialog).getByText('“notes.csv” could not be read as notes.')).toBeInTheDocument();
    expect(within(dialog).getByRole('alert')).toHaveTextContent('The CSV file needs a title column and a body column.');
    expect(within(dialog).getByText(/A CSV file needs the columns title and body/)).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Choose another file' })).toBeEnabled();
  });

  it('goes back to the format on request', async () => {
    fakeMemory(undefined, undefined, undefined, undefined, undefined, { preview: refused('missingColumns') });
    const dialog = await openReview();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Show the format' }));

    expect(within(dialog).getByText('Up to 4000 characters.')).toBeInTheDocument();
    expect(within(dialog).queryByRole('alert')).toBeNull();
    expect(within(dialog).getByRole('button', { name: 'Choose file…' })).toBeEnabled();
  });

  it('names the line a CSV file breaks at', async () => {
    fakeMemory(undefined, undefined, undefined, undefined, undefined, { preview: refused('invalidCsv', 14) });
    const dialog = await openReview();

    expect(within(dialog).getByRole('alert')).toHaveTextContent('The file is not valid CSV. It breaks at line 14.');
  });

  it('asks for the picker again and reviews the next file', async () => {
    const files = [refused('tooLarge'), SAMPLE_IMPORT];
    const invoke = fakeMemory(undefined, undefined, undefined, undefined, undefined, { get preview() { return files.shift() ?? null; } });
    const dialog = await openReview();

    expect(within(dialog).getByRole('alert')).toHaveTextContent('The file is larger than 2 MiB.');
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Choose another file' }));

    expect(await within(dialog).findByText('team-notes.csv · CSV file · 5 notes')).toBeInTheDocument();
    expect(calls(invoke, 'memory_notes_import_preview')).toHaveLength(2);
  });
});

describe('the entry points', () => {
  it('opens the format from the menu, without the picker', async () => {
    const invoke = fakeMemory();
    await openStart();

    expect(calls(invoke, 'memory_notes_import_preview')).toEqual([]);
    expect(toasts.items).toEqual([]);
  });

  it('is offered by the empty list too', async () => {
    const invoke = fakeMemory(NO_NOTES, undefined, undefined, []);
    nav.go('notes');
    render(NotesView);

    await fireEvent.click(await screen.findByRole('button', { name: 'Import from file' }));

    expect(await screen.findByRole('dialog', { name: 'Import notes' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Choose file…' })).toBeEnabled();
    expect(calls(invoke, 'memory_notes_import_preview')).toEqual([]);
  });

  it('is not offered while Harness Memory cannot save notes', async () => {
    fakeMemory({ kind: 'outdated' });
    nav.go('notes');
    render(NotesView);
    await screen.findByText('Your memory needs an update');

    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));

    expect(await screen.findByRole('menuitem', { name: /Delete all notes/ })).toBeInTheDocument();
    expect(screen.queryByRole('menuitem', { name: 'Import from file…' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Import from file' })).toBeNull();
  });
});

describe('a review that outlives nothing', () => {
  afterEach(async () => {
    await project.open(PROJECT);
  });

  it('is dropped when the view is left', async () => {
    fakeMemory();
    await openReview();

    cleanup();

    expect(notesImport.open).toBe(false);
    expect(notesImport.plan).toBeNull();
  });

  it('is dropped when the project changes, and the dialog closes', async () => {
    fakeMemory();
    await openReview();

    project.active = null;

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Import notes' })).not.toBeInTheDocument());
    expect(notesImport.plan).toBeNull();
    expect(notesImport.picked).toEqual([]);
  });

  it('does not open for a file that was picked before the review was dropped', async () => {
    const invoke = fakeMemory();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;
    let release = () => {};
    const held = new Promise<void>((resolve) => (release = resolve));
    invoke.mockImplementation((async (name: CommandName, args: never) => {
      if (name === 'memory_notes_import_preview') {
        await held;
      }

      return real(name as never, args);
    }) as typeof ipc.invoke);

    const choosing = notesImport.choose(PROJECT);
    notesImport.reset();
    release();
    await choosing;

    expect(notesImport.open).toBe(false);
    expect(notesImport.plan).toBeNull();
    expect(notesImport.picking).toBe(false);
  });
});
