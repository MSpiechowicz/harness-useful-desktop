import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { defaultScenario } from '$lib/ipc/mock/scenarios';
import { createMockTransport } from '$lib/ipc/mock/transport';
import { branchSync } from '$lib/stores/branch-sync.svelte';
import { git } from '$lib/stores/git.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { chatSession, startedSession } from '../../../../tests/chat-helpers';
import BranchChip from './BranchChip.svelte';
import BranchPopover from './BranchPopover.svelte';
import { chip, note, openMenu, PROJECT, useBranchMenu } from './branch-menu-support';

useBranchMenu();

describe('the branch chip', () => {
  it('shows the branch with what is ahead and behind, and that main moved on', async () => {
    await chip('?sync=behind');

    const button = screen.getByRole('button', { name: /^Branch / });
    expect(button).toHaveTextContent('feat/empty-input');
    expect(button).toHaveAccessibleName(/2 ahead.*main has 7 new commits/);
    expect(button).toHaveAttribute('aria-haspopup', 'dialog');
  });

  it('names the branch being rebased, never a detached HEAD, and says the rebase is stopped', async () => {
    await chip('?sync=conflict');

    const button = screen.getByRole('button', { name: /^Branch / });
    expect(button).toHaveTextContent('feat/empty-input');
    expect(button).toHaveTextContent('rebasing');
    expect(button).not.toHaveTextContent('detached');
  });

  it('is not there without a git repository', async () => {
    const folder = '/home/demo/Notes/research';
    const backend = createMockTransport(defaultScenario(), 0);
    vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) => backend.invoke(name as never, args as never)) as typeof ipc.invoke);
    git.activate(folder);
    await waitFor(() => expect(git.loadStatus).toBe('ready'));

    render(BranchChip, {
      session: chatSession([], { project: folder }).session,
    });

    expect(screen.queryByRole('button', { name: /^Branch / })).toBeNull();
  });
});

describe('how the branch stands', () => {
  it('shows the remote and main rows with their actions', async () => {
    await chip('?sync=behind');
    const dialog = await openMenu();

    expect(within(dialog).getByText('origin/feat/empty-input')).toBeInTheDocument();
    expect(within(dialog).getByText('2 local commits not pushed')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Push 2' })).toBeEnabled();
    expect(within(dialog).getByText('7 new commits since you branched')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Rebase on main' })).toBeEnabled();
    expect(within(dialog).getByText(/Checked origin/)).toBeInTheDocument();
  });

  it('offers Publish for a branch the remote does not have, and hides the remote for a repository without one', async () => {
    await chip('?sync=unpublished');
    const dialog = await openMenu();
    expect(within(dialog).getByText('Not on the remote yet')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Publish' })).toBeEnabled();
  });

  it('says nothing about a remote when there is none', async () => {
    await chip('?sync=noremote');
    const dialog = await openMenu();

    expect(within(dialog).queryByText(/Checked/)).toBeNull();
    expect(within(dialog).queryByRole('button', { name: /^(Push|Publish|Pull)/ })).toBeNull();
  });

  it('says when the remote could not be reached, without a toast', async () => {
    await chip('?sync=offline');
    const dialog = await openMenu();

    expect(await within(dialog).findByText('Couldn’t reach origin')).toBeInTheDocument();
    expect(within(dialog).queryByText(/Checked/)).toBeNull();
    expect(toasts.items).toEqual([]);
  });

  it('offers Pull when the branch is behind its remote', async () => {
    await chip('?sync=onmain');
    const dialog = await openMenu();

    expect(within(dialog).getByText('3 new commits on the remote')).toBeInTheDocument();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Pull' }));

    expect(await note(dialog, 'Pulled new commits from origin/main')).toBeInTheDocument();
    expect(within(dialog).getByText('In sync with the remote')).toBeInTheDocument();
  });

  it('leaves out the main row on main itself', async () => {
    await chip('?sync=onmain');
    const dialog = await openMenu();

    expect(within(dialog).queryByRole('button', { name: /Rebase on/ })).toBeNull();
  });
});

describe('updating from main', () => {
  it('rebases with one click and reports it', async () => {
    await chip('?sync=behind');
    const dialog = await openMenu();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Rebase on main' }));

    expect(await note(dialog, 'Rebased on main')).toBeInTheDocument();
    expect(within(dialog).getByText('No new commits')).toBeInTheDocument();
  });

  it('merges from the menu next to the button, which opens inside the same panel', async () => {
    await chip('?sync=behind');
    const dialog = await openMenu();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'More ways to update' }));
    const menu = within(dialog).getByRole('menu');
    expect(
      within(menu)
        .getAllByRole('menuitem')
        .map((item) => item.textContent?.replace(/\s+/g, ' ').trim()),
    ).toEqual(['Rebase on main Replays your commits on top', 'Merge main Adds one merge commit', 'Pull feat/empty-input Fast-forward from the remote only']);

    await fireEvent.click(within(menu).getByRole('menuitem', { name: /Merge main/ }));

    expect(await note(dialog, 'Merged main')).toBeInTheDocument();
  });

  it('warns that uncommitted files are stashed first and put back after', async () => {
    await chip('?sync=dirty');
    const dialog = await openMenu();

    expect(within(dialog).getByText('3 files are not committed. Rebase stashes them first and puts them back after.')).toBeInTheDocument();
  });

  it('keeps the other buttons off while an action runs', async () => {
    await chip('?sync=behind');
    const dialog = await openMenu();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Push 2' }));

    expect(within(dialog).getByRole('button', { name: 'Rebase on main' })).toBeDisabled();
    expect(await note(dialog, 'Pushed to origin/feat/empty-input')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Rebase on main' })).toBeEnabled();
  });
});

describe('a rebase that stopped', () => {
  it('lists the conflicts in place of the remote and main rows, with two equal buttons', async () => {
    await chip('?sync=conflict');
    const dialog = await openMenu();

    expect(within(dialog).getByText('Rebase onto main stopped')).toBeInTheDocument();
    expect(within(dialog).getByText('3 files conflict')).toBeInTheDocument();
    expect(within(dialog).queryByText('origin/feat/empty-input')).toBeNull();

    const ask = within(dialog).getByRole('button', { name: 'Ask Oh My Pi' });
    const abort = within(dialog).getByRole('button', { name: 'Abort rebase' });
    expect(ask.parentElement).toBe(abort.parentElement);
    expect(ask.parentElement).toHaveClass('grid-cols-2');
  });

  it('opens a conflicting file of the project in Changes, and leaves one outside the project alone', async () => {
    await chip('?sync=conflict');
    const dialog = await openMenu();

    expect(within(dialog).getByTitle('Outside this project folder')).toBeInTheDocument();
    expect(within(dialog).queryByRole('button', { name: /git\.rs/ })).toBeNull();

    await fireEvent.click(
      within(dialog).getByRole('button', {
        name: 'Open ui/src/features/chat/ComposerFooter.svelte in Changes',
      }),
    );

    expect(nav.current).toBe('changes');
    expect(git.selected).toBe('ui/src/features/chat/ComposerFooter.svelte');
  });

  it('hands the conflicts to the harness of this chat', async () => {
    const chat = await chip('?sync=conflict');
    const dialog = await openMenu();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Ask Oh My Pi' }));

    const prompts = () => chat.of('agent_send').filter((call) => call.action.type === 'prompt');
    await waitFor(() => expect(prompts()).toHaveLength(1));
    expect(JSON.stringify(prompts()[0].action)).toContain('ui/src/features/chat/ComposerFooter.svelte');
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Branch' })).toBeNull());
  });

  it('puts the branch back with Abort', async () => {
    await chip('?sync=conflict');
    const dialog = await openMenu();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Abort rebase' }));

    expect(await within(dialog).findByText('Rebase aborted, your branch is back where it was')).toBeInTheDocument();
    expect(within(dialog).queryByText('Rebase onto main stopped')).toBeNull();
    expect(within(dialog).getByText('origin/feat/empty-input')).toBeInTheDocument();
  });

  it('offers to continue, not to ask, once no conflict is left', async () => {
    await chip('?sync=resolved');
    const dialog = await openMenu();

    expect(within(dialog).getByText('All conflicts are resolved')).toBeInTheDocument();
    expect(within(dialog).queryByRole('button', { name: /^Ask/ })).toBeNull();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Continue rebase' }));

    expect(await within(dialog).findByText('Rebase finished')).toBeInTheDocument();
  });

  it('calls a stopped merge a merge', async () => {
    await chip('?sync=merge');
    const dialog = await openMenu();

    expect(within(dialog).getByText('Merge of main stopped')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Abort merge' })).toBeInTheDocument();
  });

  it('stops a rebase that hits conflicts when it is clicked', async () => {
    await chip('?sync=clash');
    const dialog = await openMenu();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Rebase on main' }));

    expect(await within(dialog).findByText('Rebase onto main stopped')).toBeInTheDocument();
  });
});

describe('the main branch', () => {
  it('opens a picker in the same panel, says where the main branch came from and saves a pick', async () => {
    await chip();
    const dialog = await openMenu();

    await fireEvent.click(
      within(dialog).getByRole('button', {
        name: /Choose the main branch: main/,
      }),
    );
    expect(within(dialog).getByText('Guessed from branch names')).toBeInTheDocument();

    await fireEvent.click(
      await within(dialog).findByRole('menuitemradio', {
        name: /fix\/hooks-first-message/,
      }),
    );

    expect(
      await within(dialog).findByRole('button', {
        name: /Choose the main branch: fix\/hooks-first-message/,
      }),
    ).toBeInTheDocument();

    await fireEvent.click(within(dialog).getByRole('button', { name: /Choose the main branch/ }));
    expect(within(dialog).getByText('Picked by you')).toBeInTheDocument();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Use detected' }));
    expect(
      await within(dialog).findByRole('button', {
        name: /Choose the main branch: main/,
      }),
    ).toBeInTheDocument();
  });

  it('asks for a choice when none was found', async () => {
    await chip('?sync=nomain');
    const dialog = await openMenu();

    expect(within(dialog).getByText('No main branch found')).toBeInTheDocument();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Choose' }));
    expect(within(dialog).getByText('Main branch')).toBeInTheDocument();
  });
});

describe('opening the popover', () => {
  it('keeps a note that arrives right after it opened when the state updates', async () => {
    await chip('?sync=behind');
    const dialog = await openMenu();

    branchSync.note = { tone: 'success', text: 'Fresh note' };
    branchSync.state = { ...structuredClone(branchSync.state!), ahead: 5 };

    expect(await within(dialog).findByText('Fresh note')).toBeInTheDocument();
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(within(dialog).getByText('Fresh note')).toBeInTheDocument();
  });

  it('says it could not read the repository, and still lists the branches', async () => {
    history.replaceState(null, '', '/');
    const backend = createMockTransport(defaultScenario(), 0);
    vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) => {
      if (name === 'git_sync_state') {
        return Promise.reject(new Error('boom'));
      }

      return backend.invoke(name as never, args as never);
    }) as typeof ipc.invoke);
    git.activate(PROJECT);
    await waitFor(() => expect(git.status).not.toBeNull());
    branchSync.follow(PROJECT, null);
    await waitFor(() => expect(branchSync.readFailed).toBe(true));

    const chat = await startedSession();
    render(BranchPopover, {
      session: chat.session,
      close: () => {},
      ondrop: () => {},
    });

    expect(await screen.findByText('Couldn’t read this repository')).toBeInTheDocument();
    expect(await screen.findAllByRole('menuitemradio')).not.toHaveLength(0);
    expect(document.querySelector('[role="progressbar"], .animate-spin')).toBeNull();
  });
});
