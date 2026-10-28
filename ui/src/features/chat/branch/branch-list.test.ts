import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { branchSync } from '$lib/stores/branch-sync.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { chip, note, openMenu, useBranchMenu } from './branch-menu-support';

useBranchMenu();

afterEach(() => vi.unstubAllGlobals());

describe('the branches', () => {
  it('lists them with the current one marked, and finds one on the server as you type', async () => {
    await chip();
    const dialog = await openMenu();

    const rows = await within(dialog).findAllByRole('menuitemradio');
    expect(rows.find((row) => row.getAttribute('aria-checked') === 'true')).toHaveTextContent('feat/empty-input');
    expect(
      within(dialog).getByRole('menuitemradio', {
        name: /feat\/opencode-sessions/,
      }),
    ).toHaveTextContent('origin');

    await fireEvent.input(within(dialog).getByRole('searchbox', { name: 'Search branches' }), { target: { value: 'memory' } });

    await waitFor(() => expect(within(dialog).getAllByRole('menuitemradio')).toHaveLength(1));
    expect(
      within(dialog).getByRole('menuitemradio', {
        name: /feat\/memory-search-v2/,
      }),
    ).toBeInTheDocument();
  });

  it('shows a page of a long list and loads the next one when the end comes into view', async () => {
    const watchers: (() => void)[] = [];
    vi.stubGlobal(
      'IntersectionObserver',
      class {
        constructor(private readonly callback: IntersectionObserverCallback) {}
        observe() {
          watchers.push(() => this.callback([{ isIntersecting: true } as IntersectionObserverEntry], this as unknown as IntersectionObserver));
        }
        disconnect() {}
      },
    );
    await chip('?branches=1500');
    const dialog = await openMenu();

    await waitFor(() => expect(within(dialog).getAllByRole('menuitemradio')).toHaveLength(50));
    expect(within(dialog).queryByText(/refine the search/)).not.toBeInTheDocument();

    // The fetch the popover starts may land meanwhile and search again, which starts the list over: scrolling asks again.
    await waitFor(() => {
      watchers.at(-1)?.();
      expect(within(dialog).getAllByRole('menuitemradio')).toHaveLength(100);
    });
  });

  it('asks again for a page that failed when the list is scrolled near its end', async () => {
    await chip('?branches=1500');
    const dialog = await openMenu();
    await waitFor(() => expect(within(dialog).getAllByRole('menuitemradio')).toHaveLength(50));
    const list = within(dialog).getByRole('group', { name: 'Branches' });
    Object.defineProperties(list, { scrollHeight: { value: 1800 }, clientHeight: { value: 200 }, scrollTop: { value: 1500, writable: true } });
    const loadMore = vi.spyOn(branchSync, 'loadMore').mockResolvedValue();

    await fireEvent.scroll(list);
    expect(loadMore).not.toHaveBeenCalled();

    branchSync.searchFailed = true;
    await fireEvent.scroll(list);
    expect(loadMore).toHaveBeenCalledOnce();

    list.scrollTop = 0;
    await fireEvent.scroll(list);
    expect(loadMore).toHaveBeenCalledOnce();
    loadMore.mockRestore();
    branchSync.searchFailed = false;
  });

  it('switches branch on a click', async () => {
    await chip();
    const dialog = await openMenu();

    await fireEvent.click(
      await within(dialog).findByRole('menuitemradio', {
        name: /fix\/hooks-first-message/,
      }),
    );

    expect(await note(dialog, 'Switched to fix/hooks-first-message')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^Branch / })).toHaveTextContent('fix/hooks-first-message');
  });

  it('offers nothing to create while only searching', async () => {
    await chip('?sync=dirty');
    const dialog = await openMenu();

    await fireEvent.input(within(dialog).getByRole('searchbox', { name: 'Search branches' }), { target: { value: 'no-such-branch' } });

    await waitFor(() => expect(within(dialog).queryByRole('button', { name: /Create no-such-branch/ })).not.toBeInTheDocument());
  });

  it('creates a named branch from main with Enter after New branch, bringing the uncommitted files along', async () => {
    await chip('?sync=dirty');
    const dialog = await openMenu();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'New branch' }));
    const field = within(dialog).getByRole('searchbox', {
      name: 'Search branches',
    });

    await fireEvent.input(field, { target: { value: 'feat new thing' } });

    expect(
      await within(dialog).findByRole('button', {
        name: /Create feat-new-thing/,
      }),
    ).toBeInTheDocument();
    expect(within(dialog).getByText('Your 4 uncommitted files come along')).toBeInTheDocument();
    expect(within(dialog).getByRole('radio', { name: 'main' })).toBeChecked();

    await fireEvent.keyDown(field, { key: 'Enter' });

    expect(await note(dialog, 'Created feat-new-thing from main')).toHaveTextContent('Your 4 uncommitted files came along');
    expect(screen.getByRole('button', { name: /^Branch / })).toHaveTextContent('feat-new-thing');
  });

  it('starts the new branch from the current one when asked to', async () => {
    await chip('?sync=behind');
    const dialog = await openMenu();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'New branch' }));
    await fireEvent.click(within(dialog).getByRole('radio', { name: 'this branch' }));
    await fireEvent.input(within(dialog).getByRole('searchbox', { name: 'Search branches' }), { target: { value: 'experiment' } });
    await fireEvent.click(await within(dialog).findByRole('button', { name: /Create experiment/ }));

    expect(await note(dialog, 'Created experiment from feat/empty-input')).toBeInTheDocument();
    await waitFor(() => expect(within(dialog).getByRole('button', { name: 'New branch' })).toBeEnabled());
  });

  it('offers Stash and switch when git refuses to switch over uncommitted files', async () => {
    await chip('?sync=dirty');
    const dialog = await openMenu();

    await fireEvent.click(await within(dialog).findByRole('menuitemradio', { name: /^main/ }));

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0].action?.label).toBe('Stash and switch');

    toasts.items[0].action?.run();

    expect(await within(dialog).findByText('Your changes are kept in the stash')).toBeInTheDocument();
    const apply = within(dialog).getByRole('button', { name: 'Apply stash' });
    await waitFor(() => expect(apply).toBeEnabled());
    await fireEvent.click(apply);
    expect(await within(dialog).findByText('Your changes are back')).toBeInTheDocument();
    expect(within(dialog).queryByText('Your changes are kept in the stash')).toBeNull();
  });

  it('shows a stash that is waiting from the start', async () => {
    await chip('?sync=stash');
    const dialog = await openMenu();

    expect(within(dialog).getByText('Your changes are kept in the stash')).toBeInTheDocument();
  });
});
