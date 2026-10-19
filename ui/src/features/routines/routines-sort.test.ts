import { fireEvent, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { viewState } from '$lib/stores/view-state.svelte';

import { listed } from '../../../tests/doc-support';
import { fakeRoutines, registerRoutinesHooks, showRoutines } from './routines-test-support';

registerRoutinesHooks();

const GLOBAL = '# Global  4 of 64';
const PROJECT = '# This project demo 4 of 64';

const grid = () => screen.getByRole('grid', { name: 'Routines' });
const header = (name: string) => within(grid()).getByRole('columnheader', { name });
const sortBy = (name: string) => fireEvent.click(within(header(name)).getByRole('button'));

describe('sorting the routines table', () => {
  it('starts in the run order, with every sortable header unsorted', async () => {
    fakeRoutines();
    await showRoutines();

    for (const name of ['Routine', 'When', 'Next run', 'Status', 'On']) {
      expect(header(name)).toHaveAttribute('aria-sort', 'none');
    }
    expect(viewState.routinesSort).toBeNull();
    expect(listed(grid())).toEqual([GLOBAL, 'Check the disk', 'Morning review', 'Look for hogs', 'Weekly changelog', PROJECT, 'Release notes', 'Check the build on start', 'Run the tests', 'Triage new issues']);
  });

  it('sorts the routines of each group by a column and reverses on the second click, the group headings staying where they are', async () => {
    fakeRoutines();
    await showRoutines();

    await sortBy('Routine');

    expect(header('Routine')).toHaveAttribute('aria-sort', 'ascending');
    expect(listed(grid())).toEqual([GLOBAL, 'Check the disk', 'Look for hogs', 'Morning review', 'Weekly changelog', PROJECT, 'Check the build on start', 'Release notes', 'Run the tests', 'Triage new issues']);

    await sortBy('Routine');

    expect(header('Routine')).toHaveAttribute('aria-sort', 'descending');
    expect(listed(grid())).toEqual([GLOBAL, 'Weekly changelog', 'Morning review', 'Look for hogs', 'Check the disk', PROJECT, 'Triage new issues', 'Run the tests', 'Release notes', 'Check the build on start']);
    expect(viewState.routinesSort).toEqual({ key: 'routine', desc: true });
  });

  it('orders by the next run with the routines that have none last, whichever way it runs', async () => {
    fakeRoutines();
    await showRoutines();

    await sortBy('Next run');
    expect(listed(grid())).toEqual([GLOBAL, 'Look for hogs', 'Check the disk', 'Morning review', 'Weekly changelog', PROJECT, 'Run the tests', 'Triage new issues', 'Release notes', 'Check the build on start']);

    await sortBy('Next run');
    expect(header('Next run')).toHaveAttribute('aria-sort', 'descending');
    expect(listed(grid())).toEqual([GLOBAL, 'Morning review', 'Check the disk', 'Look for hogs', 'Weekly changelog', PROJECT, 'Triage new issues', 'Run the tests', 'Release notes', 'Check the build on start']);
  });

  it('orders by the kind of schedule, and by On with the switched-on routines first', async () => {
    fakeRoutines();
    await showRoutines();

    await sortBy('When');
    expect(listed(grid()).slice(6)).toEqual(['Release notes', 'Run the tests', 'Triage new issues', 'Check the build on start']);

    await sortBy('On');
    expect(header('On')).toHaveAttribute('aria-sort', 'descending');
    expect(header('When')).toHaveAttribute('aria-sort', 'none');
    expect(listed(grid()).slice(6)).toEqual(['Run the tests', 'Triage new issues', 'Release notes', 'Check the build on start']);
  });

  it('is sorted from the keyboard: a header is a button', async () => {
    fakeRoutines();
    await showRoutines();

    const button = within(header('Status')).getByRole('button');
    button.focus();
    expect(button).toHaveFocus();
    await fireEvent.click(button);

    expect(header('Status')).toHaveAttribute('aria-sort', 'ascending');
  });
});

describe('the badges of the routines table', () => {
  it('are the larger size, as in the Harness family dialog', async () => {
    fakeRoutines();
    await showRoutines();

    for (const badge of ['Active', 'Off']) {
      expect(within(grid()).getAllByText(badge)[0]).toHaveClass('h-7');
    }
    expect(within(grid()).getAllByText('Unattended')[0]).toHaveClass('h-7');
  });
});
