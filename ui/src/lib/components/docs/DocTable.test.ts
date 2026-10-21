import { cleanup, fireEvent, render, screen, within } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import DocScopeCell from './DocScopeCell.svelte';
import DocTable from './DocTable.svelte';

afterEach(cleanup);

const COLUMNS = [{ key: 'name', label: 'Name' }];

const cells = createRawSnippet<[string]>((view) => ({ render: () => `<td role="gridcell">${view()}</td>` }));

function flat(rows: string[], extra: Record<string, unknown> = {}) {
  const onopen = vi.fn();
  const onmove = vi.fn();

  render(DocTable<string>, { label: 'Docs', columns: COLUMNS, rows, rowKey: (view: string) => view, openId: null, onopen, onmove, cells, ...extra });

  return { onopen, onmove };
}

describe('a flat documents table', () => {
  it('lists the rows without group headings and opens one on click or Enter', async () => {
    const { onopen } = flat(['one', 'two']);
    const rows = within(screen.getByRole('grid', { name: 'Docs' })).getAllByRole('row').slice(1);

    expect(rows.map((row) => row.textContent)).toEqual(['one', 'two']);

    await fireEvent.click(rows[1]);
    await fireEvent.keyDown(rows[0], { key: 'Enter' });

    expect(onopen.mock.calls).toEqual([['two'], ['one']]);
  });

  it('moves between rows with the arrow keys and keeps one Tab stop, on the open row when there is one', async () => {
    const { onmove } = flat(['one', 'two', 'three'], { openId: 'two' });
    const rows = within(screen.getByRole('grid', { name: 'Docs' })).getAllByRole('row').slice(1);

    expect(rows.map((row) => row.tabIndex)).toEqual([-1, 0, -1]);

    await fireEvent.keyDown(rows[1], { key: 'ArrowDown' });
    expect(onmove).toHaveBeenCalledWith('three');
    expect(rows[2]).toHaveFocus();
  });

  it('puts the pager under the rows and turns the page with it', async () => {
    const onpage = vi.fn();
    flat(['one', 'two'], { pagination: { page: 0, pageSize: 2, total: 5, onpage } });

    expect(screen.getByText('1–2 of 5')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(onpage).toHaveBeenCalledWith(1);
  });

  it('has no pager unless asked for', () => {
    flat(['one']);

    expect(screen.queryByRole('navigation', { name: 'Pages' })).not.toBeInTheDocument();
  });
});

describe('the Scope cell', () => {
  function cell(scope: { kind: 'global' } | { kind: 'repo'; label: string } | { kind: 'unknown' }, remote = false) {
    render(DocScopeCell, { scope, remote });
  }

  it('names the repository of a row, or says Global for the global ones and Unknown for a file that could not be read', () => {
    cell({ kind: 'repo', label: 'acme/widget' });
    expect(screen.getByRole('gridcell')).toHaveTextContent('acme/widget');
    cleanup();

    cell({ kind: 'global' });
    expect(screen.getByRole('gridcell')).toHaveTextContent('Global');
    cleanup();

    cell({ kind: 'unknown' });
    expect(screen.getByRole('gridcell')).toHaveTextContent('Unknown');
  });

  it('marks a team row Remote and leaves the others unmarked', () => {
    cell({ kind: 'repo', label: 'acme/widget' }, true);
    expect(screen.getByRole('gridcell')).toHaveTextContent('Remote');
    cleanup();

    cell({ kind: 'repo', label: 'acme/widget' });
    expect(screen.getByRole('gridcell')).not.toHaveTextContent('Remote');
  });
});
