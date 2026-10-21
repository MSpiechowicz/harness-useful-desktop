import { fireEvent, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { viewState } from '$lib/stores/view-state.svelte';

import { listed } from '../../../tests/doc-support';
import { fakeHooks, registerHooksHooks, showHooks } from './hooks-test-support';

registerHooksHooks();

const grid = () => screen.getByRole('grid', { name: 'Hooks' });
const header = (name: string) => within(grid()).getByRole('columnheader', { name });
const sortBy = (name: string) => fireEvent.click(within(header(name)).getByRole('button'));

describe('sorting the hooks table', () => {
  it('starts in the run order, with every sortable header unsorted', async () => {
    fakeHooks();
    await showHooks();

    for (const name of ['Hook', 'Scope', 'When', 'Does', 'Status', 'On']) {
      expect(header(name)).toHaveAttribute('aria-sort', 'none');
    }
    expect(viewState.hooksSort).toBeNull();
  });

  it('sorts all the hooks as one list by a column and reverses on the second click', async () => {
    fakeHooks();
    await showHooks();

    expect(listed(grid())).toEqual(['Search Harness Memory first', 'Save what you learned', 'Run lint after edits', 'Remind about tests']);

    await sortBy('Hook');

    expect(header('Hook')).toHaveAttribute('aria-sort', 'ascending');
    expect(listed(grid())).toEqual(['Remind about tests', 'Run lint after edits', 'Save what you learned', 'Search Harness Memory first']);

    await sortBy('Hook');

    expect(header('Hook')).toHaveAttribute('aria-sort', 'descending');
    expect(listed(grid())).toEqual(['Search Harness Memory first', 'Save what you learned', 'Run lint after edits', 'Remind about tests']);
    expect(viewState.hooksSort).toEqual({ key: 'hook', desc: true });
  });

  it('sorts by the scope with Global first, keeping the run order among the hooks of one scope', async () => {
    fakeHooks();
    await showHooks();

    await sortBy('Scope');
    expect(header('Scope')).toHaveAttribute('aria-sort', 'ascending');
    expect(listed(grid())).toEqual(['Search Harness Memory first', 'Save what you learned', 'Run lint after edits', 'Remind about tests']);

    await sortBy('Scope');
    expect(header('Scope')).toHaveAttribute('aria-sort', 'descending');
    expect(listed(grid())).toEqual(['Run lint after edits', 'Remind about tests', 'Search Harness Memory first', 'Save what you learned']);
  });

  it('sorts by the status as the reader sees it, and by On with the switched-on hooks first, keeping the run order among equals', async () => {
    fakeHooks();
    await showHooks();

    await sortBy('Status');
    expect(header('Status')).toHaveAttribute('aria-sort', 'ascending');
    expect(listed(grid())).toEqual(['Search Harness Memory first', 'Save what you learned', 'Remind about tests', 'Run lint after edits']);

    await sortBy('On');
    expect(header('On')).toHaveAttribute('aria-sort', 'descending');
    expect(header('Status')).toHaveAttribute('aria-sort', 'none');
    expect(listed(grid())).toEqual(['Search Harness Memory first', 'Save what you learned', 'Run lint after edits', 'Remind about tests']);

    await sortBy('On');
    expect(header('On')).toHaveAttribute('aria-sort', 'ascending');
    expect(listed(grid())).toEqual(['Run lint after edits', 'Remind about tests', 'Search Harness Memory first', 'Save what you learned']);
  });

  it('starts again from the first page when the sort changes', async () => {
    fakeHooks();
    await showHooks();
    viewState.hooksPage = 3;

    await sortBy('Hook');

    expect(viewState.hooksPage).toBe(0);
  });

  it('is sorted from the keyboard: a header is a button', async () => {
    fakeHooks();
    await showHooks();

    const button = within(header('Status')).getByRole('button');
    button.focus();
    expect(button).toHaveFocus();
    await fireEvent.click(button);

    expect(header('Status')).toHaveAttribute('aria-sort', 'ascending');
  });
});

describe('the badges of the hooks table', () => {
  it('are the larger size, as in the Harness family dialog', async () => {
    fakeHooks();
    await showHooks();

    for (const badge of ['Active', 'Off', 'Pending approval']) {
      expect(within(grid()).getAllByText(badge)[0]).toHaveClass('h-7');
    }
  });
});
