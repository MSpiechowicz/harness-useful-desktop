import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { restoreClientWidth, stubClientWidth } from '../../../tests/helpers';
import { fakeMemory, noteRow, pickFilter, registerMemoryHooks, rowButton, showMemory, showNotes } from './memory-test-support';

registerMemoryHooks();

afterEach(() => {
  restoreClientWidth();
  vi.restoreAllMocks();
});

const WIDE = 1600;
const NARROW = 1000;

describe('a narrow view', () => {
  it('keeps the filters above the table and slides the panel in over the view, with no empty detail beside it', async () => {
    stubClientWidth(NARROW);
    fakeMemory();
    await showMemory();

    expect(screen.getByRole('button', { name: 'Scope: All' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Status: All' })).toBeInTheDocument();
    expect(screen.queryByText('Select a script to see its code')).toBeNull();
    expect(screen.queryByRole('region')).toBeNull();

    await fireEvent.click(rowButton('open-prs') as HTMLElement);

    expect(await screen.findByRole('region', { name: 'open-prs' })).toBeInTheDocument();
  });

  it('says how much there is once, in the row of the filters', async () => {
    stubClientWidth(NARROW);
    fakeMemory();
    await showMemory();

    expect(screen.getAllByText('5 scripts')).toHaveLength(1);

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search scripts' }), { target: { value: 'pull requests' } });

    expect(await screen.findByText('1 script')).toBeInTheDocument();
    expect(screen.queryByText('5 scripts')).toBeNull();
  });

  it('counts the notes in the same place', async () => {
    stubClientWidth(NARROW);
    fakeMemory();
    await showNotes();

    expect(screen.getAllByText('70 notes')).toHaveLength(1);
  });
});

describe('a wide view', () => {
  it('lays out the table and a detail column that asks for a choice until there is one, with the same filters above the table', async () => {
    stubClientWidth(WIDE);
    fakeMemory();
    await showMemory();

    // No column of choices beside the table: the filters are the same two menus in the row above it.
    expect(screen.queryByRole('radiogroup')).toBeNull();
    expect(screen.getByRole('button', { name: 'Scope: All' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Status: All' })).toBeInTheDocument();
    expect(screen.getByText('Select a script to see its code')).toBeInTheDocument();
    expect(screen.getAllByText('5 scripts')).toHaveLength(1);
  });

  it('narrows the table from the menu of the filter', async () => {
    stubClientWidth(WIDE);
    fakeMemory();
    await showMemory();

    await pickFilter('Scope: All', /^Global/);

    await waitFor(() => expect(rowButton('open-prs')).toBeNull());
    expect(rowButton('disk-hogs')).not.toBeNull();
  });

  it('shows the note to read in the detail column in place of the request to choose one', async () => {
    stubClientWidth(WIDE);
    fakeMemory();
    await showNotes();

    expect(screen.getByText('Select a note to read it')).toBeInTheDocument();
    expect(screen.getAllByText('70 notes')).toHaveLength(1);

    await fireEvent.click(noteRow(3) as HTMLElement);

    expect(await screen.findByRole('region', { name: 'Notes are read through the Memory CLI, never its database' })).toBeInTheDocument();
    expect(screen.queryByText('Select a note to read it')).toBeNull();
  });

  it('swaps the detail when another row is chosen and goes back to the request when it is closed, with the focus on the row', async () => {
    stubClientWidth(WIDE);
    fakeMemory();
    await showMemory();

    await fireEvent.click(rowButton('open-prs') as HTMLElement);
    expect(await screen.findByRole('region', { name: 'open-prs' })).toBeInTheDocument();

    await fireEvent.click(rowButton('ci-status') as HTMLElement);
    expect(await screen.findByRole('region', { name: 'ci-status' })).toBeInTheDocument();
    expect(screen.queryByRole('region', { name: 'open-prs' })).toBeNull();

    await fireEvent.click(within(screen.getByRole('region', { name: 'ci-status' })).getByRole('button', { name: 'Close' }));

    expect(await screen.findByText('Select a script to see its code')).toBeInTheDocument();
    expect(document.activeElement).toBe(rowButton('ci-status'));
  });

  it('browses with the arrow keys and closes on Escape', async () => {
    stubClientWidth(WIDE);
    fakeMemory();
    await showMemory();
    const first = rowButton('open-prs') as HTMLElement;
    first.focus();

    await fireEvent.keyDown(first, { key: 'Enter' });
    await screen.findByRole('region', { name: 'open-prs' });
    await fireEvent.keyDown(first, { key: 'ArrowDown' });

    expect(await screen.findByRole('region', { name: 'ci-status' })).toBeInTheDocument();
    expect(document.activeElement).toBe(rowButton('ci-status'));

    await fireEvent.keyDown(document.activeElement as HTMLElement, { key: 'Escape' });

    expect(await screen.findByText('Select a script to see its code')).toBeInTheDocument();
    expect(document.activeElement).toBe(rowButton('ci-status'));
  });
});

describe('the rows', () => {
  it('end with a chevron that tells they open something', async () => {
    fakeMemory();
    await showMemory();

    const cells = (rowButton('open-prs') as HTMLElement).querySelectorAll('td');
    const last = cells[cells.length - 1];

    expect(last).toHaveAttribute('aria-hidden', 'true');
    expect(last.querySelector('svg')).not.toBeNull();
  });
});

describe('moving between rows', () => {
  // `scrollIntoView` scrolls every clipping ancestor up to the page, which slides the whole shell with its top bar.
  it('never scrolls anything outside the table', async () => {
    const scrolled = vi.fn();
    Element.prototype.scrollIntoView = scrolled;
    fakeMemory();
    await showMemory();
    const first = rowButton('open-prs') as HTMLElement;
    first.focus();

    await fireEvent.keyDown(first, { key: 'Enter' });
    await screen.findByRole('region', { name: 'open-prs' });
    await fireEvent.keyDown(first, { key: 'ArrowDown' });
    await fireEvent.keyDown(document.activeElement as HTMLElement, { key: 'End' });
    await fireEvent.keyDown(document.activeElement as HTMLElement, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('region')).toBeNull());

    expect(document.activeElement).toBe(rowButton('disk-hogs'));
    expect(scrolled).not.toHaveBeenCalled();
  });
});
