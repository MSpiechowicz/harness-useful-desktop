import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { projectChoice } from '$lib/components/scope/scope-options';
import type { HookDraft } from '$lib/ipc/bindings/HookDraft';
import type { MockHooks } from '$lib/ipc/mock/hooks-state';
import { project } from '$lib/stores/project.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { listed } from '../../../tests/doc-support';
import { hooks } from './hooks-data.svelte';
import { appliesTo, calls, fakeHooks, localRowOf, OTHER_PROJECT, pick, pickTarget, PROJECT, registerHooksHooks, rowOf, showHooks, teamRowOf } from './hooks-test-support';

registerHooksHooks();

const grid = () => screen.getByRole('grid', { name: 'Hooks' });
const RESEARCH = projectChoice({ path: OTHER_PROJECT, key: `path:${OTHER_PROJECT}` });

/** A hook of the user's in `project` (a folder, null for every project), written the way the editor does. */
function hookIn(state: MockHooks, name: string, project: string | null): string {
  const draft: HookDraft = {
    id: null,
    expectedSha256: null,
    name,
    description: null,
    scope: project === null ? 'global' : 'project',
    project,
    event: 'sessionStart',
    matcher: null,
    action: { type: 'addContext', text: 'Look at the notes.' },
    enabled: true,
  };

  return state.save(draft).id;
}

const menuItems = (menu: HTMLElement) => within(menu).getAllByRole('menuitemradio').map((item) => item.textContent?.replace(/\s+/g, ' ').trim());

describe('The Scope filter of the hooks', () => {
  it('starts on All, listing the hooks of every project, the global ones and the remote ones of other repositories, first in the row of filters', async () => {
    const { state } = fakeHooks('team');
    hookIn(state, 'Notes for research', OTHER_PROJECT);
    await showHooks();

    expect(screen.getAllByRole('button', { name: /^(Scope|When|Status|Source):/ })[0]).toHaveAccessibleName('Scope: All');
    expect(viewState.hooksScope).toBe('all');
    expect(within(localRowOf('Notes for research')).getByText('research')).toBeInTheDocument();
    expect(within(localRowOf('Run lint after edits')).getByText('demo')).toBeInTheDocument();
    expect(within(localRowOf('Search Harness Memory first')).getByText('Global')).toBeInTheDocument();
    expect(within(teamRowOf('Lint the stylesheets')).getByText('acme/website')).toBeInTheDocument();
    expect(within(teamRowOf('Lint the stylesheets')).getByText('Remote')).toBeInTheDocument();
  });

  it('lists the projects with how many hooks each has, the repositories only the team has, and Global', async () => {
    const { state } = fakeHooks('team');
    hookIn(state, 'Notes for research', OTHER_PROJECT);
    await showHooks();

    await fireEvent.click(screen.getByRole('button', { name: 'Scope: All' }));

    expect(menuItems(await screen.findByRole('menu', { name: 'Scope' }))).toEqual(['All 13', 'demo useful/harness-desktop 4', 'legacy-api 0', 'research 1', 'acme/website Remote only 1', 'Global 7']);
  });

  it('narrows to one project, the hooks of the team for that repository included', async () => {
    const { state } = fakeHooks('team');
    hookIn(state, 'Notes for research', OTHER_PROJECT);
    await showHooks();

    await pick('Scope: All', /^demo/);

    expect(viewState.hooksScope).toBe(projectChoice({ path: PROJECT, key: 'git:github.com/useful/harness-desktop' }));
    expect(screen.getByRole('button', { name: 'Scope: demo' })).toBeInTheDocument();
    expect(listed(grid())).toEqual(expect.arrayContaining(['Run lint after edits', 'Remind about tests', 'Run the type checker']));
    expect(listed(grid())).not.toContain('Notes for research');
    expect(listed(grid())).not.toContain('Search Harness Memory first');
    expect(listed(grid())).not.toContain('Lint the stylesheets');
  });

  it('narrows to a repository only the team has, and to the global hooks', async () => {
    const { state } = fakeHooks('team');
    hookIn(state, 'Notes for research', OTHER_PROJECT);
    await showHooks();

    await pick('Scope: All', /^acme\/website/);
    expect(listed(grid())).toEqual(['Lint the stylesheets']);
    expect(screen.getByText('1 hook')).toBeInTheDocument();

    await pick('Scope: acme/website', /^Global/);
    expect(listed(grid())).toContain('Search Harness Memory first');
    expect(listed(grid())).not.toContain('Lint the stylesheets');
    expect(listed(grid())).not.toContain('Run lint after edits');
  });

  it('says so when a project has no hooks, and clears with its x', async () => {
    fakeHooks();
    await showHooks();

    await pick('Scope: All', /^research/);
    expect(await screen.findByText('No hooks match these filters.')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Scope filter' }));
    expect(viewState.hooksScope).toBe('all');
    expect(rowOf('Search Harness Memory first')).toBeInTheDocument();
  });

  it('keeps the choice when the view is left and shown again', async () => {
    fakeHooks();
    viewState.hooksScope = 'global';
    await showHooks();

    expect(screen.getByRole('button', { name: 'Scope: Global' })).toBeInTheDocument();
    expect(listed(grid())).not.toContain('Run lint after edits');
  });
});

describe('The pages of the hooks table', () => {
  it('hold 50 hooks and turn with the pager', async () => {
    const { state } = fakeHooks();

    for (let at = 0; at < 60; at += 1) {
      hookIn(state, `Hook number ${String(at).padStart(2, '0')}`, null);
    }

    await showHooks();
    await hooks.refresh();

    expect(listed(grid())).toHaveLength(50);
    expect(screen.getByText('1–50 of 64')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));

    expect(listed(grid())).toHaveLength(14);
    expect(screen.getByText('51–64 of 64')).toBeInTheDocument();
    expect(viewState.hooksPage).toBe(1);
  });

  it('start again from the first page when the filter, the search or the scope changes', async () => {
    const { state } = fakeHooks();

    for (let at = 0; at < 60; at += 1) {
      hookIn(state, `Hook number ${String(at).padStart(2, '0')}`, null);
    }

    await showHooks();
    await hooks.refresh();

    for (const change of [() => pick('Status: All', /^Active/), () => pick('Scope: All', /^Global/), () => fireEvent.input(screen.getByRole('searchbox', { name: 'Search hooks' }), { target: { value: 'number' } })]) {
      viewState.hooksPage = 1;
      await change();
      await waitFor(() => expect(viewState.hooksPage).toBe(0));
    }
  });

  it('keep the last page when the hooks shrink under it', async () => {
    fakeHooks();
    viewState.hooksPage = 4;
    await showHooks();

    expect(screen.getByText('1–4 of 4')).toBeInTheDocument();
    expect(listed(grid())).toHaveLength(4);
  });
});

describe('The hooks of a project that is not open', () => {
  it('are switched by their own project, not the open one', async () => {
    const { state, invoke } = fakeHooks();
    const id = hookIn(state, 'Notes for research', OTHER_PROJECT);
    await showHooks();

    await fireEvent.click(within(rowOf('Notes for research')).getByRole('switch', { name: 'Turn off Notes for research' }));

    await waitFor(() => expect(within(rowOf('Notes for research')).getByText('Off')).toBeInTheDocument());
    expect(calls(invoke, 'hook_save')).toEqual([expect.objectContaining({ draft: expect.objectContaining({ id, scope: 'project', project: OTHER_PROJECT, enabled: false }) })]);
  });

  it('name their own project in the panel and in the question before they are deleted, never This project', async () => {
    const { state } = fakeHooks();
    hookIn(state, 'Notes for research', OTHER_PROJECT);
    await showHooks();

    await fireEvent.click(rowOf('Notes for research'));
    const panel = await screen.findByRole('region', { name: 'Notes for research' });

    expect(within(panel).getByText('Session start · research')).toBeInTheDocument();
    expect(within(within(panel).getByRole('heading', { name: 'Details' }).closest('section') as HTMLElement).getByText('research')).toBeInTheDocument();
    expect(within(panel).queryByText(/This project/)).not.toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Delete' }));

    expect(within(await screen.findByRole('dialog', { name: 'Delete this hook?' })).getByText('Session start · research')).toBeInTheDocument();
  });

  it('are edited in their own project, which the Applies to field shows', async () => {
    const { state, invoke } = fakeHooks();
    hookIn(state, 'Notes for research', OTHER_PROJECT);
    await showHooks();

    await fireEvent.click(rowOf('Notes for research'));
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Notes for research' })).getByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit hook' });

    expect(appliesTo(dialog)).toHaveTextContent('research');
    expect(within(dialog).getByText('Runs only in research.')).toBeInTheDocument();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'hook_save')).toHaveLength(1));
    expect(calls(invoke, 'hook_save')[0]).toMatchObject({ draft: { scope: 'project', project: OTHER_PROJECT } });
  });

  it('can be moved to another project or to every project from the editor', async () => {
    const { state, invoke } = fakeHooks();
    hookIn(state, 'Notes for research', OTHER_PROJECT);
    await showHooks();

    await fireEvent.click(rowOf('Notes for research'));
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Notes for research' })).getByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit hook' });

    await pickTarget(dialog, /^demo/);
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'hook_save')).toHaveLength(1));
    expect(calls(invoke, 'hook_save')[0]).toMatchObject({ draft: { scope: 'project', project: PROJECT } });
    await waitFor(() => expect(within(rowOf('Notes for research')).getByText('demo')).toBeInTheDocument());
  });

  it('are deleted, approved and shared by their id alone', async () => {
    const { state, invoke } = fakeHooks('team');
    const id = hookIn(state, 'Notes for research', OTHER_PROJECT);
    await showHooks();

    await fireEvent.click(rowOf('Notes for research'));
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Notes for research' })).getByRole('button', { name: 'Share with team' }));

    await waitFor(() => expect(calls(invoke, 'hooks_share')).toHaveLength(1));
    expect(calls(invoke, 'hooks_share')[0]).toEqual({ id, sha256: expect.stringMatching(/^[0-9a-f]{64}$/) });
  });
});

describe('The editor of a new hook', () => {
  async function openEditor(): Promise<HTMLElement> {
    await fireEvent.click(screen.getAllByRole('button', { name: 'New hook' })[0]);

    return screen.findByRole('dialog', { name: 'New hook' });
  }

  async function fillAndAdd(dialog: HTMLElement): Promise<void> {
    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Mind the budget' } });
    await fireEvent.input(within(dialog).getByLabelText('Context'), { target: { value: 'Check the limits first.' } });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add hook' }));
  }

  it('starts for every project while the Scope filter shows All, and offers Global and every project on this computer', async () => {
    fakeHooks();
    await showHooks();
    const dialog = await openEditor();

    expect(appliesTo(dialog)).toHaveTextContent('Global');
    expect(within(dialog).getByText('Runs in every project.')).toBeInTheDocument();

    await fireEvent.click(appliesTo(dialog));
    const menu = await screen.findByRole('menu', { name: 'Applies to' });

    expect(menuItems(menu)).toEqual(['Global', 'demo useful/harness-desktop', 'research']);
  });

  it('starts for the project the Scope filter shows, and saves it there although it is not the open one', async () => {
    viewState.hooksScope = RESEARCH;
    const { invoke } = fakeHooks();
    await showHooks();
    const dialog = await openEditor();

    expect(appliesTo(dialog)).toHaveTextContent('research');
    expect(within(dialog).getByText('Runs only in research.')).toBeInTheDocument();

    await fillAndAdd(dialog);

    await waitFor(() => expect(calls(invoke, 'hook_save')).toHaveLength(1));
    expect(calls(invoke, 'hook_save')[0]).toMatchObject({ draft: { name: 'Mind the budget', scope: 'project', project: OTHER_PROJECT } });
  });

  it('starts for every project while the Scope filter shows Global', async () => {
    viewState.hooksScope = 'global';
    fakeHooks();
    await showHooks();
    const dialog = await openEditor();

    expect(appliesTo(dialog)).toHaveTextContent('Global');
  });

  it('saves for a recent project the user picks, and for no project at all when Global is picked', async () => {
    const { invoke } = fakeHooks();
    await showHooks();
    let dialog = await openEditor();

    await pickTarget(dialog, /^research/);
    expect(within(dialog).getByText('Runs only in research.')).toBeInTheDocument();
    await fillAndAdd(dialog);
    await waitFor(() => expect(calls(invoke, 'hook_save')).toHaveLength(1));
    expect(calls(invoke, 'hook_save')[0]).toMatchObject({ draft: { scope: 'project', project: OTHER_PROJECT } });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New hook' })).not.toBeInTheDocument());

    dialog = await openEditor();
    await pickTarget(dialog, /^demo/);
    await pickTarget(dialog, 'Global');
    await fillAndAdd(dialog);
    await waitFor(() => expect(calls(invoke, 'hook_save')).toHaveLength(2));
    expect(calls(invoke, 'hook_save')[1]).toMatchObject({ draft: { scope: 'global', project: null } });
  });

  it('counts the hooks of the chosen scope against the most it can hold, not those of the open project', async () => {
    const { state } = fakeHooks();

    for (let at = 0; at < 64; at += 1) {
      hookIn(state, `Research hook ${at}`, OTHER_PROJECT);
    }

    await showHooks();
    await hooks.refresh();
    const dialog = await openEditor();
    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'One more' } });
    await fireEvent.input(within(dialog).getByLabelText('Context'), { target: { value: 'Check.' } });

    expect(within(dialog).getByRole('button', { name: 'Add hook' })).toBeEnabled();

    await pickTarget(dialog, /^research/);

    expect(await within(dialog).findByText('This scope already holds 64 hooks, the most it can.')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Add hook' })).toBeDisabled();

    await pickTarget(dialog, /^demo/);

    await waitFor(() => expect(within(dialog).getByRole('button', { name: 'Add hook' })).toBeEnabled());
  });

  it('offers no project whose folder is gone and no repository only the team has', async () => {
    fakeHooks('team');
    await showHooks();
    const dialog = await openEditor();

    await fireEvent.click(appliesTo(dialog));
    const menu = await screen.findByRole('menu', { name: 'Applies to' });

    expect(menuItems(menu).some((item) => item?.includes('acme'))).toBe(false);
    expect(within(menu).queryByRole('menuitemradio', { name: /^legacy-api/ })).not.toBeInTheDocument();
  });
});

describe('The remote hooks of another repository', () => {
  it('cannot be copied while no project on this computer is theirs, with the reason as a tooltip', async () => {
    const { invoke } = fakeHooks('team');
    await showHooks();

    await fireEvent.click(teamRowOf('Lint the stylesheets'));
    const copy = within(await screen.findByRole('region', { name: 'Lint the stylesheets' })).getByRole('button', { name: 'Copy to my hooks' });

    expect(copy).toBeDisabled();
    await fireEvent.pointerEnter(copy.parentElement as HTMLElement);
    expect(await screen.findByRole('tooltip')).toHaveTextContent('Open this project to copy this hook to it.');
    expect(calls(invoke, 'hooks_team_copy')).toEqual([]);
  });

  it('are copied for the project of their repository, not the open one', async () => {
    await project.open(OTHER_PROJECT);

    try {
      const { invoke } = fakeHooks('teamTrusted');
      await showHooks();
      const team = hooks.views.find((view) => view.origin === 'team' && view.name === 'Run the type checker') as NonNullable<(typeof hooks.views)[number]>;

      await fireEvent.click(teamRowOf('Run the type checker'));
      await fireEvent.click(within(await screen.findByRole('region', { name: 'Run the type checker' })).getByRole('button', { name: 'Copy to my hooks' }));

      await waitFor(() => expect(calls(invoke, 'hooks_team_copy')).toEqual([{ id: team.id, sha256: team.sha256, project: PROJECT }]));
      expect(calls(invoke, 'hooks_list')[0]).toEqual({ project: OTHER_PROJECT, all: true });
    } finally {
      await project.open(PROJECT);
    }
  });
});
