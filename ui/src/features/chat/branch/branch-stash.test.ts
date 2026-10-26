import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { branchSync } from '$lib/stores/branch-sync.svelte';
import { git } from '$lib/stores/git.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { chip, note, openMenu, useBranchMenu } from './branch-menu-support';

useBranchMenu();

describe('a copy left in the stash', () => {
  const dropCalls = () => vi.mocked(ipc.invoke).mock.calls.filter(([name, args]) => name === 'git_sync_action' && (args as { action: string }).action === 'dropStash');

  it('says the changes are back and offers to drop the copy, not to apply it', async () => {
    await chip('?sync=stashapplied');
    const dialog = await openMenu();

    expect(await note(dialog, 'Your changes are back')).toHaveTextContent('A copy is still in the stash');
    expect(within(dialog).getByRole('button', { name: 'Drop copy' })).toBeEnabled();
    expect(within(dialog).queryByRole('button', { name: 'Apply stash' })).toBeNull();
  });

  it('closes the menu and drops the copy only after the user confirms', async () => {
    await chip('?sync=stashapplied');
    const dialog = await openMenu();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Drop copy' }));
    const confirm = await screen.findByRole('dialog', {
      name: 'Drop the copy in the stash?',
    });
    expect(dropCalls()).toHaveLength(0);

    await fireEvent.click(within(confirm).getByRole('button', { name: 'Drop copy' }));

    await waitFor(() => expect(dropCalls()).toHaveLength(1));
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toEqual(['Copy dropped']));
    expect(branchSync.state?.stashKept).toBe(false);
    expect(branchSync.state?.stashApplied).toBe(false);
  });

  it('leaves the copy alone when the user keeps it', async () => {
    await chip('?sync=stashapplied');
    const dialog = await openMenu();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Drop copy' }));
    const confirm = await screen.findByRole('dialog', {
      name: 'Drop the copy in the stash?',
    });
    await fireEvent.click(within(confirm).getByRole('button', { name: 'Keep it' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Drop the copy in the stash?' })).toBeNull());
    expect(dropCalls()).toHaveLength(0);
    expect(branchSync.state?.stashKept).toBe(true);
  });
});

describe('conflicts left by a stash', () => {
  it('shows the files and the two buttons, and offers no Apply stash while they conflict', async () => {
    await chip('?sync=stashclash');
    const dialog = await openMenu();

    expect(within(dialog).getByText('Your changes conflict with main')).toBeInTheDocument();
    expect(within(dialog).getByText('3 files conflict')).toBeInTheDocument();
    expect(within(dialog).queryByText('origin/feat/empty-input')).toBeNull();
    expect(within(dialog).queryByRole('button', { name: 'Apply stash' })).toBeNull();
    expect(within(dialog).queryByText('Your changes are kept in the stash')).toBeNull();

    const ask = within(dialog).getByRole('button', { name: 'Ask Oh My Pi' });
    const open = within(dialog).getByRole('button', {
      name: 'Open in Changes',
    });
    expect(ask.parentElement).toBe(open.parentElement);
    expect(ask.parentElement).toHaveClass('grid-cols-2');
    expect(within(dialog).queryByRole('button', { name: /^Abort/ })).toBeNull();
  });

  it('opens the first file of the project in Changes, and keeps the branch list from switching', async () => {
    await chip('?sync=stashclash');
    const dialog = await openMenu();

    expect(within(dialog).getByRole('button', { name: 'New branch' })).toBeDisabled();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Open in Changes' }));

    expect(nav.current).toBe('changes');
    expect(git.selected).toBe('ui/src/features/chat/ComposerFooter.svelte');
  });

  it('asks the harness to resolve them and drop only the stash the app made', async () => {
    const chat = await chip('?sync=stashclash');
    const dialog = await openMenu();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Ask Oh My Pi' }));

    const prompts = () => chat.of('agent_send').filter((call) => call.action.type === 'prompt');
    await waitFor(() => expect(prompts()).toHaveLength(1));
    const text = JSON.stringify(prompts()[0].action);
    expect(text).toContain('git stash drop');
    expect(text).toContain('Harness Desktop:');
    expect(text).toContain('Never drop any other stash');
  });
});
