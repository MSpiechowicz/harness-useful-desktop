import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { hooks } from './hooks-data.svelte';
import { calls, fakeHooks, localRowOf, pick, PROJECT, registerHooksHooks, rowOf, showHooks, teamRowOf } from './hooks-test-support';

registerHooksHooks();

const TYPES = 'Run the type checker';
const COMMITS = 'Use conventional commits';
const STATUS = 'Show the git status when a chat ends';
const SMALL = 'Keep commits small';

/** Opens a hook's panel from its row and answers the panel. */
async function open(row: HTMLElement, name: string): Promise<HTMLElement> {
  await fireEvent.click(row);

  return screen.findByRole('region', { name });
}

describe('The Team group', () => {
  it('comes after the local groups, with the hooks of the team and of this repository', async () => {
    fakeHooks('team');
    await showHooks();

    const grid = screen.getByRole('grid', { name: 'Hooks' });
    const headings = within(grid).getAllByRole('columnheader').filter((cell) => cell.getAttribute('scope') === 'colgroup');

    expect(headings.map((cell) => cell.textContent?.replace(/\s+/g, ' ').trim())).toEqual(['Global 4 of 64', 'This project demo 2 of 64', 'Team 5']);
    expect(within(teamRowOf(COMMITS)).getByText('Active')).toBeInTheDocument();
    expect(within(teamRowOf(TYPES)).getByText('Pending approval')).toBeInTheDocument();
    expect(within(teamRowOf('Remind about tests')).getByText('Overridden')).toBeInTheDocument();
  });

  it('is not there while hooks are not shared with a team', async () => {
    const { invoke } = fakeHooks('ok');
    await showHooks();

    expect(screen.queryByText('Team', { selector: 'th span' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Source:/ })).not.toBeInTheDocument();
    expect(calls(invoke, 'hooks_team_refresh')).toEqual([{ project: PROJECT }]);
  });

  it('asks the team for its hooks when the view opens, without a refresh button', async () => {
    const { invoke } = fakeHooks('team');
    await showHooks();

    await waitFor(() => expect(calls(invoke, 'hooks_team_refresh')).toEqual([{ project: PROJECT }]));
    expect(screen.queryByRole('button', { name: /refresh/i })).not.toBeInTheDocument();
  });

  it('narrows to the team or to the hooks of this computer with a third filter after When and Status', async () => {
    fakeHooks('team');
    await showHooks();

    const buttons = screen.getAllByRole('button', { name: /^(When|Status|Source):/ }).map((button) => button.getAttribute('aria-label'));
    expect(buttons).toEqual(['When: All', 'Status: All', 'Source: All']);

    await pick('Source: All', /^Team/);
    expect(screen.getByRole('button', { name: 'Source: Team' })).toBeInTheDocument();
    expect(screen.queryByText('Search Harness Memory first')).not.toBeInTheDocument();
    expect(teamRowOf(COMMITS)).toBeInTheDocument();

    await pick('Source: Team', /^Mine/);
    expect(screen.getByText('Search Harness Memory first')).toBeInTheDocument();
    expect(document.querySelector('[data-row^="team:"]')).toBeNull();
    expect(screen.queryByText('Team', { selector: 'th span' })).not.toBeInTheDocument();
  });

  it('lists Overridden as a status only while a team hook is in it', async () => {
    fakeHooks('team');
    await showHooks();

    await fireEvent.click(screen.getByRole('button', { name: 'Status: All' }));
    const menu = await screen.findByRole('menu', { name: 'Status' });
    expect(within(menu).getByRole('menuitemradio', { name: /^Overridden/ })).toHaveTextContent('2');
  });

  it('says quietly that the latest refresh failed, in the group', async () => {
    fakeHooks('teamProblem');
    await showHooks();

    expect(await screen.findByText(/Could not refresh the team hooks, so these are the last ones read\. The team database did not answer in time\./)).toBeInTheDocument();
    expect(teamRowOf(COMMITS)).toBeInTheDocument();
  });
});

describe('The trust of the team', () => {
  it('is said in the Team group while team hooks run without asking, and not otherwise', async () => {
    fakeHooks('teamTrusted');
    await showHooks();

    expect(screen.getByText('Trusted: run without asking')).toBeInTheDocument();
  });

  it('is not claimed while approval is needed', async () => {
    fakeHooks('team');
    await showHooks();

    expect(screen.queryByText('Trusted: run without asking')).not.toBeInTheDocument();
  });
});

describe('Sharing a local hook', () => {
  it('puts it on the team, where it shows as overridden by the original', async () => {
    const { invoke } = fakeHooks('team');
    await showHooks();
    const summarize = hooks.views.find((view) => view.name === 'Summarize the diff') as NonNullable<(typeof hooks.views)[number]>;

    await fireEvent.click(within(await open(rowOf('Summarize the diff'), 'Summarize the diff')).getByRole('button', { name: 'Share with team' }));

    await waitFor(() => expect(within(teamRowOf('Summarize the diff')).getByText('Overridden')).toBeInTheDocument());
    expect(calls(invoke, 'hooks_share')).toEqual([{ id: summarize.id, sha256: summarize.sha256 }]);
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Summarize the diff shared with the team.'));
    expect(within(screen.getByRole('region', { name: 'Summarize the diff' })).getByRole('button', { name: 'Share with team' })).toBeDisabled();
  });

  it('is off for a hook of the Harness family, and for one that waits for approval', async () => {
    const { invoke } = fakeHooks('team');
    await showHooks();

    expect(within(await open(rowOf('Search Harness Memory first'), 'Search Harness Memory first')).getByRole('button', { name: 'Share with team' })).toBeDisabled();
    expect(within(await open(rowOf('Run lint after edits'), 'Run lint after edits')).getByRole('button', { name: 'Share with team' })).toBeDisabled();
    expect(calls(invoke, 'hooks_share')).toEqual([]);
  });

  it('is off for a project hook in a repository without an origin remote', async () => {
    fakeHooks('teamNoOrigin');
    await showHooks();

    const panel = await open(rowOf('Remind about tests'), 'Remind about tests');
    const share = within(panel).getByRole('button', { name: 'Share with team' });

    expect(share).toBeDisabled();
    await fireEvent.pointerEnter(share.parentElement as HTMLElement);
    expect(await screen.findByRole('tooltip')).toHaveTextContent('This repository has no origin remote, so its hooks cannot be shared.');
    expect(screen.queryByText(TYPES)).not.toBeInTheDocument();
  });

  it('is not offered while hooks are not shared', async () => {
    fakeHooks('ok');
    await showHooks();

    const panel = await open(rowOf('Remind about tests'), 'Remind about tests');

    expect(within(panel).getByRole('button', { name: 'Edit' })).toBeInTheDocument();
    expect(within(panel).queryByRole('button', { name: 'Share with team' })).not.toBeInTheDocument();
  });
});

describe('A team hook', () => {
  it('shows who shared it and when, and can neither be edited nor deleted', async () => {
    fakeHooks('team');
    await showHooks();

    const panel = await open(teamRowOf(COMMITS), COMMITS);
    const details = within(panel).getByText('Details').closest('section') as HTMLElement;

    expect(within(details).getByText('Shared by')).toBeInTheDocument();
    expect(within(details).getByText('Anna Kowalska')).toBeInTheDocument();
    expect(within(details).getByText('Updated on the team')).toBeInTheDocument();
    expect(within(details).getByText('Team', { selector: 'dd, span, p, div' })).toBeInTheDocument();
    expect(within(panel).queryByRole('button', { name: 'Edit' })).not.toBeInTheDocument();
    expect(within(panel).queryByRole('button', { name: 'Delete' })).not.toBeInTheDocument();
  });

  it('is switched on and off for this computer only, and the panel says so', async () => {
    const { invoke } = fakeHooks('team');
    await showHooks();
    const team = hooks.views.find((view) => view.origin === 'team' && view.name === COMMITS) as NonNullable<(typeof hooks.views)[number]>;

    const panel = await open(teamRowOf(COMMITS), COMMITS);
    expect(within(panel).getByText(/This switch only changes this computer\./)).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('switch', { name: `Run ${COMMITS} on this computer` }));

    await waitFor(() => expect(within(teamRowOf(COMMITS)).getByText('Off')).toBeInTheDocument());
    expect(calls(invoke, 'hooks_team_enable')).toEqual([{ id: team.id, enabled: false }]);
    expect(calls(invoke, 'hook_save')).toEqual([]);

    await fireEvent.click(within(teamRowOf(COMMITS)).getByRole('switch', { name: `Turn on ${COMMITS}` }));
    await waitFor(() => expect(within(teamRowOf(COMMITS)).getByText('Active')).toBeInTheDocument());
  });

  it('is copied to the hooks of the user, who can then edit the copy', async () => {
    const { invoke } = fakeHooks('team');
    await showHooks();
    const team = hooks.views.find((view) => view.origin === 'team' && view.name === COMMITS) as NonNullable<(typeof hooks.views)[number]>;

    await fireEvent.click(within(await open(teamRowOf(COMMITS), COMMITS)).getByRole('button', { name: 'Copy to my hooks' }));

    await waitFor(() => expect(within(localRowOf(COMMITS)).getByText('Active')).toBeInTheDocument());
    expect(calls(invoke, 'hooks_team_copy')).toEqual([{ id: team.id, sha256: team.sha256, project: PROJECT }]);
    expect(within(teamRowOf(COMMITS)).getByText('Overridden')).toBeInTheDocument();
    await waitFor(() => expect(viewState.hookId).toBe(team.id));
    expect(within(await screen.findByRole('region', { name: COMMITS })).getByRole('button', { name: 'Edit' })).toBeEnabled();
  });

  it('is removed from the team only after a confirm', async () => {
    const { invoke } = fakeHooks('team');
    await showHooks();
    const team = hooks.views.find((view) => view.origin === 'team' && view.name === COMMITS) as NonNullable<(typeof hooks.views)[number]>;

    await fireEvent.click(within(await open(teamRowOf(COMMITS), COMMITS)).getByRole('button', { name: 'Remove from team' }));
    const dialog = await screen.findByRole('dialog', { name: 'Remove this hook from the team?' });
    expect(calls(invoke, 'hooks_team_remove')).toEqual([]);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Keep it' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Remove this hook from the team?' })).not.toBeInTheDocument());
    expect(calls(invoke, 'hooks_team_remove')).toEqual([]);

    await fireEvent.click(within(screen.getByRole('region', { name: COMMITS })).getByRole('button', { name: 'Remove from team' }));
    await fireEvent.click(within(await screen.findByRole('dialog', { name: 'Remove this hook from the team?' })).getByRole('button', { name: 'Remove from team' }));

    await waitFor(() => expect(document.querySelector('[data-row^="team:"]')?.textContent).not.toContain(COMMITS));
    expect(calls(invoke, 'hooks_team_remove')).toEqual([{ id: team.id, sha256: team.sha256 }]);
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain(`${COMMITS} removed from the team.`));
  });

  it('offers to update its team copy from the original when the original changed since it was shared', async () => {
    const { invoke } = fakeHooks('team');
    await showHooks();
    const original = hooks.views.find((view) => view.origin === 'local' && view.name === SMALL) as NonNullable<(typeof hooks.views)[number]>;

    const panel = await open(teamRowOf(SMALL), SMALL);
    await fireEvent.click(within(panel).getByRole('button', { name: 'Update team copy' }));

    await waitFor(() => expect(within(screen.getByRole('region', { name: SMALL })).queryByRole('button', { name: 'Update team copy' })).not.toBeInTheDocument());
    expect(calls(invoke, 'hooks_share')).toEqual([{ id: original.id, sha256: original.sha256 }]);
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain(`Team copy of ${SMALL} updated.`));
  });

  it('has no update while the original is as it was shared', async () => {
    fakeHooks('team');
    await showHooks();

    const panel = await open(teamRowOf('Remind about tests'), 'Remind about tests');

    expect(within(panel).queryByRole('button', { name: 'Update team copy' })).not.toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Copy to my hooks' })).toBeInTheDocument();
  });

  it('says why a hook of the team does not run when the user has one with the same id', async () => {
    fakeHooks('team');
    await showHooks();

    const row = teamRowOf('Remind about tests');
    expect(within(row).getByRole('switch')).toBeDisabled();

    const panel = await open(row, 'Remind about tests');
    const status = within(panel).getByText('Status').closest('section') as HTMLElement;

    expect(within(status).getByText('Overridden')).toBeInTheDocument();
    expect(within(status).queryByText('On')).not.toBeInTheDocument();
    expect(within(status).getByText('You have a hook with the same id, so yours runs instead of this team copy.')).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Copy to my hooks' })).toBeDisabled();
  });

  it('says in its details when the original changed since it was shared', async () => {
    fakeHooks('team');
    await showHooks();

    const panel = await open(teamRowOf(SMALL), SMALL);
    const details = within(panel).getByText('Details').closest('section') as HTMLElement;

    expect(within(details).getByText('Your hook')).toBeInTheDocument();
    expect(within(details).getByText('Changed since you shared it')).toBeInTheDocument();
  });
});

describe('A team hook that waits for approval', () => {
  it('is never offered for copying, and the backend refuses it too', async () => {
    const { invoke, state } = fakeHooks('team');
    await showHooks();
    const team = hooks.views.find((view) => view.origin === 'team' && view.name === STATUS) as NonNullable<(typeof hooks.views)[number]>;

    const copy = within(await open(teamRowOf(STATUS), STATUS)).getByRole('button', { name: 'Copy to my hooks' });
    expect(copy).toBeDisabled();

    await fireEvent.pointerEnter(copy.parentElement as HTMLElement);
    expect(await screen.findByRole('tooltip')).toHaveTextContent('Approve it first.');

    await fireEvent.click(copy);
    expect(calls(invoke, 'hooks_team_copy')).toEqual([]);
    expect(() => state.teamCopy(team.id, team.sha256, PROJECT)).toThrow(/waits for your approval/);
  });

  it('shows who changed it and what, against the version approved, and cannot be rejected', async () => {
    fakeHooks('team');
    await showHooks();

    const panel = await open(teamRowOf(TYPES), TYPES);
    const review = within(panel).getByRole('status', { name: '' });

    expect(within(review).getByText('Marek Nowak changed this team hook')).toBeInTheDocument();
    expect(within(review).getByRole('group', { name: 'Changes to Tool filter' })).toHaveTextContent('edit|write');
    expect(within(review).getByRole('figure', { name: 'Changes to Command' })).toHaveTextContent('bun run check && bun run lint');
    expect(within(review).getByRole('button', { name: 'Approve' })).toBeEnabled();
    expect(within(panel).queryByRole('button', { name: /^Reject/ })).not.toBeInTheDocument();
  });

  it('says who shared a new one, and runs once the user approves exactly that version', async () => {
    const { invoke } = fakeHooks('team');
    await showHooks();
    const team = hooks.views.find((view) => view.origin === 'team' && view.name === STATUS) as NonNullable<(typeof hooks.views)[number]>;

    const panel = await open(teamRowOf(STATUS), STATUS);
    expect(within(panel).getByText('Anna Kowalska shared this hook with the team')).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Approve' }));

    await waitFor(() => expect(within(teamRowOf(STATUS)).getByText('Active')).toBeInTheDocument());
    expect(calls(invoke, 'hook_approve')).toEqual([{ id: team.id, sha256: team.sha256 }]);
    expect(calls(invoke, 'hook_reject')).toEqual([]);
  });

  it('keeps its switch off, as a click would not approve anything', async () => {
    fakeHooks('team');
    await showHooks();

    const toggle = within(teamRowOf(TYPES)).getByRole('switch');

    expect(toggle).toBeDisabled();
    expect(toggle).toHaveAttribute('aria-checked', 'false');
  });

  it('runs without approval while the team is trusted', async () => {
    fakeHooks('teamTrusted');
    await showHooks();

    expect(within(teamRowOf(TYPES)).getByText('Active')).toBeInTheDocument();
    expect(within(teamRowOf(STATUS)).getByText('Active')).toBeInTheDocument();
    expect(hooks.pending).toBe(1);
  });
});
