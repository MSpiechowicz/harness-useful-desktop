import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';

import { fakeMemory, registerMemoryHooks, showNotes } from './memory-test-support';

registerMemoryHooks();

const STORAGE_KEY = 'harness:table:memory-notes';

/** happy-dom lays nothing out: every element is this wide, so a column and the room the flexible one can spare are known. */
const COLUMN_PX = 200;

beforeEach(() => {
  Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, get: () => COLUMN_PX });
  HTMLElement.prototype.setPointerCapture = vi.fn();
  localStorage.clear();
});

afterEach(() => {
  Reflect.deleteProperty(HTMLElement.prototype, 'offsetWidth');
  Reflect.deleteProperty(HTMLElement.prototype, 'setPointerCapture');
  Reflect.deleteProperty(HTMLElement.prototype, 'clientWidth');
  localStorage.clear();
  document.documentElement.removeAttribute('style');
});

const handle = (column: string) => screen.getByRole('separator', { name: `Resize the ${column} column` });
const header = (column: string) => screen.getByRole('columnheader', { name: column });
const widthOf = (column: string) => header(column).style.width;

describe('resizing a column', () => {
  it('gives every column but the flexible title a handle, labelled with its name', async () => {
    fakeMemory();
    await showNotes();

    for (const column of ['Kind', 'Scope', 'Kept in', 'Updated']) {
      expect(handle(column)).toHaveAttribute('aria-orientation', 'vertical');
    }
    expect(screen.queryByRole('separator', { name: 'Resize the Title column' })).toBeNull();
  });

  it('grows a column by dragging its edge, keeps the width, and does not select text or sort', async () => {
    fakeMemory();
    await showNotes();
    const edge = handle('Kind');

    await fireEvent.pointerDown(edge, { clientX: 100, pointerId: 1 });
    expect(document.documentElement.style.userSelect).toBe('none');
    expect(document.documentElement.style.cursor).toBe('col-resize');

    // The column sits right of the flexible title, so its edge is on its left: moving it left makes the column wider.
    await fireEvent.pointerMove(edge, { clientX: 70, pointerId: 1 });
    expect(widthOf('Kind')).toBe('230px');

    await fireEvent.pointerUp(edge, { clientX: 70, pointerId: 1 });
    expect(document.documentElement.style.userSelect).toBe('');
    expect(document.documentElement.style.cursor).toBe('');
    expect(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}')).toEqual({ kind: 230 });
    expect(header('Updated')).toHaveAttribute('aria-sort', 'descending');
  });

  it('never goes below the narrowest width, nor takes more than the flexible column can spare', async () => {
    fakeMemory();
    await showNotes();
    const edge = handle('Scope');

    await fireEvent.pointerDown(edge, { clientX: 100, pointerId: 1 });
    await fireEvent.pointerMove(edge, { clientX: 1000, pointerId: 1 });
    expect(widthOf('Scope')).toBe('64px');

    await fireEvent.pointerMove(edge, { clientX: -1000, pointerId: 1 });
    // 200px of its own and 40px that the title can spare above its own least width.
    expect(widthOf('Scope')).toBe('240px');
    await fireEvent.pointerUp(edge, { pointerId: 1 });
  });

  it('is resized by the arrow keys, 16px at a time, the edge moving the way the key points', async () => {
    fakeMemory();
    await showNotes();

    // The edge of Kind is on its left: moving it left makes the column wider, and right narrower.
    await fireEvent.keyDown(handle('Kind'), { key: 'ArrowLeft' });
    expect(widthOf('Kind')).toBe('216px');
    expect(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}')).toEqual({ kind: 216 });

    await fireEvent.keyDown(handle('Kind'), { key: 'ArrowRight' });
    expect(widthOf('Kind')).toBe('184px');
  });

  it('goes back to its own width on a double click, and forgets the one kept', async () => {
    fakeMemory();
    await showNotes();
    await fireEvent.keyDown(handle('Kind'), { key: 'ArrowLeft' });
    expect(widthOf('Kind')).toBe('216px');

    await fireEvent.dblClick(handle('Kind'));

    expect(widthOf('Kind')).toBe('9rem');
    expect(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}')).toEqual({});
  });

  it('comes back with the widths it was left with, and survives a store that cannot be read', async () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ kind: 180, gone: 90, scope: 10 }));
    fakeMemory();
    await showNotes();

    expect(widthOf('Kind')).toBe('180px');
    // Narrower than allowed, or not a column of this table: not applied.
    expect(widthOf('Scope')).toBe('12rem');
  });

  it('shows the table with its own widths when what is kept is not JSON', async () => {
    localStorage.setItem(STORAGE_KEY, '{nope');
    fakeMemory();
    await showNotes();

    expect(widthOf('Kind')).toBe('9rem');
  });
});

describe('a filter with nothing in it', () => {
  const WIDE = 1600;

  it('stays chosen for the notes, names itself in the empty table, and gives way to All kinds on a click', async () => {
    Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => WIDE });
    fakeMemory({ kind: 'available' }, undefined, undefined, sampleNotes().filter((note) => note.kind !== 'location'));
    await showNotes();
    const kinds = screen.getByRole('radiogroup', { name: 'Filter notes by kind' });

    await fireEvent.click(within(kinds).getByRole('radio', { name: /^Locations 0/ }));

    expect(await screen.findByText('No notes match “Locations”.')).toBeInTheDocument();
    expect(within(kinds).getByRole('radio', { name: /^Locations 0/ })).toHaveAttribute('aria-checked', 'true');
    expect(within(kinds).getByRole('radio', { name: /^All kinds/ })).toHaveAttribute('aria-checked', 'false');

    await fireEvent.click(within(kinds).getByRole('radio', { name: /^All kinds/ }));

    expect(await screen.findByRole('grid', { name: 'Notes' })).toBeInTheDocument();
  });

  it('has the focus on the option that is chosen, never on another', async () => {
    Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => WIDE });
    fakeMemory();
    await showNotes();
    const kinds = screen.getByRole('radiogroup', { name: 'Filter notes by kind' });

    await fireEvent.click(within(kinds).getByRole('radio', { name: /^Fixes/ }));

    await waitFor(() => expect(within(kinds).getByRole('radio', { name: /^Fixes/ })).toHaveAttribute('aria-checked', 'true'));
    expect(document.activeElement).toBe(within(kinds).getByRole('radio', { name: /^Fixes/ }));
  });

  it('keeps a search that finds nothing on its own message, even with a kind chosen', async () => {
    Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => WIDE });
    fakeMemory();
    await showNotes();
    await fireEvent.click(screen.getByRole('radio', { name: /^Fixes/ }));
    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search notes' }), { target: { value: 'zzz' } });

    expect(await screen.findByText('No note matches that search.')).toBeInTheDocument();
  });
});
