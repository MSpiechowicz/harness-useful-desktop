import { fireEvent, screen, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { viewState } from '$lib/stores/view-state.svelte';

import { listed } from '../../../tests/doc-support';
import { fakeRoutines, registerRoutinesHooks, showRoutines } from './routines-test-support';

registerRoutinesHooks();

const WHEN_ORDER = ['Check the disk', 'Release notes', 'Morning review', 'Run the tests', 'Look for hogs', 'Weekly changelog', 'Triage new issues', 'Check the build on start'];
const ON_ORDER = ['Check the disk', 'Morning review', 'Look for hogs', 'Run the tests', 'Triage new issues', 'Weekly changelog', 'Release notes', 'Check the build on start'];

const grid = () => screen.getByRole('grid', { name: 'Routines' });
const header = (name: string) => within(grid()).getByRole('columnheader', { name });
const sortBy = (name: string) => fireEvent.click(within(header(name)).getByRole('button'));

describe('sorting the routines table', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('starts in the run order, with every sortable header unsorted', async () => {
    fakeRoutines();
    await showRoutines();

    for (const name of ['Routine', 'Scope', 'When', 'Next run', 'Status', 'On']) {
      expect(header(name)).toHaveAttribute('aria-sort', 'none');
    }
    expect(viewState.routinesSort).toBeNull();
    expect(listed(grid())).toEqual(['Check the disk', 'Morning review', 'Look for hogs', 'Weekly changelog', 'Release notes', 'Check the build on start', 'Run the tests', 'Triage new issues']);
  });

  it('sorts the routines by a column and reverses on the second click', async () => {
    fakeRoutines();
    await showRoutines();

    await sortBy('Routine');

    expect(header('Routine')).toHaveAttribute('aria-sort', 'ascending');
    expect(listed(grid())).toEqual(['Check the build on start', 'Check the disk', 'Look for hogs', 'Morning review', 'Release notes', 'Run the tests', 'Triage new issues', 'Weekly changelog']);

    await sortBy('Routine');

    expect(header('Routine')).toHaveAttribute('aria-sort', 'descending');
    expect(listed(grid())).toEqual(['Weekly changelog', 'Triage new issues', 'Run the tests', 'Release notes', 'Morning review', 'Look for hogs', 'Check the disk', 'Check the build on start']);
    expect(viewState.routinesSort).toEqual({ key: 'routine', desc: true });
  });

  it('orders by the next run with the routines that have none last, whichever way it runs', async () => {
    // A Monday morning, where the next runs come in the same order in UTC (CI) and in Central Europe (not in every zone).
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(new Date('2026-10-19T10:00:30Z'));
    fakeRoutines();
    await showRoutines();

    await sortBy('Next run');
    expect(listed(grid())).toEqual(['Run the tests', 'Triage new issues', 'Look for hogs', 'Check the disk', 'Morning review', 'Weekly changelog', 'Release notes', 'Check the build on start']);

    await sortBy('Next run');
    expect(header('Next run')).toHaveAttribute('aria-sort', 'descending');
    expect(listed(grid())).toEqual(['Morning review', 'Check the disk', 'Look for hogs', 'Triage new issues', 'Run the tests', 'Weekly changelog', 'Release notes', 'Check the build on start']);
  });

  it('orders by the kind of schedule, and by On with the switched-on routines first', async () => {
    fakeRoutines();
    await showRoutines();

    await sortBy('When');
    expect(listed(grid())).toEqual(WHEN_ORDER);

    await sortBy('On');
    expect(header('On')).toHaveAttribute('aria-sort', 'descending');
    expect(header('When')).toHaveAttribute('aria-sort', 'none');
    expect(listed(grid())).toEqual(ON_ORDER);
  });

  it('orders by the scope as the table names it, Global first and then the projects by name', async () => {
    fakeRoutines();
    await showRoutines();

    await sortBy('Scope');
    expect(header('Scope')).toHaveAttribute('aria-sort', 'ascending');
    expect(listed(grid()).slice(0, 4)).toEqual(['Check the disk', 'Morning review', 'Look for hogs', 'Weekly changelog']);

    await sortBy('Scope');
    expect(listed(grid()).slice(0, 4)).toEqual(['Release notes', 'Check the build on start', 'Run the tests', 'Triage new issues']);
    expect(viewState.routinesSort).toEqual({ key: 'scope', desc: true });
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
