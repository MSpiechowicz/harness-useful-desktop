import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import { fakeKnowledge, resetKnowledge, routedRemote, show } from './knowledge-view-support';

/** The project of the preview that is a folder without a git remote: Harness Knowledge keys it `path:/home/demo/Notes/research`. */
const FOLDER = 'research';
const REASON = 'This folder has no git remote, so teammates could not find it';

beforeAll(async () => {
  await startMockApp(fixtures.recentProjects[0].path);
});

beforeEach(() => {
  resetKnowledge();
});

afterEach(() => {
  nav.go('chat');
  toasts.items = [];
  vi.restoreAllMocks();
});

async function openLinkMenu(title: string): Promise<HTMLElement> {
  await fireEvent.click(await screen.findByText(title));
  const panel = await screen.findByRole('region', { name: title });

  await fireEvent.click(within(panel).getByRole('button', { name: 'Link a project' }));

  return panel;
}

describe('linking a document to a folder without a git remote', () => {
  it('is off with the reason for a document kept in the team database and on for one kept on this computer', async () => {
    // Both places are in use: the preview keeps every third document on this computer (the onboarding guide), the others in the team database.
    fakeKnowledge('ok', true, routedRemote('remoteLocal', 'remoteLocal'));
    await show();

    await openLinkMenu('Billing API');
    const team = await screen.findByRole('menuitem', { name: new RegExp(FOLDER) });

    expect(team).toHaveAttribute('aria-disabled', 'true');
    expect(team).toHaveAccessibleDescription(REASON);

    await fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });
    await fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });

    await openLinkMenu('Onboarding for new engineers');

    await waitFor(() => expect(screen.getAllByRole('menuitem', { name: new RegExp(FOLDER) }).at(-1)).toBeEnabled());
    expect(screen.queryByText(REASON)).toBeNull();
  });

  it('shows only the last folder of a path key, with no tooltip or label that has the whole path', async () => {
    const { store } = fakeKnowledge('ok', true, routedRemote('remoteLocal', 'remoteLocal'));
    const kept = store.documents.find((doc) => String(doc.id) === 'local:6');
    expect(kept).toBeDefined();
    (kept as NonNullable<typeof kept>).links = [{ repo: 'path:/home/ada/private/meeting-notes', globs: [] }];
    await show();

    await fireEvent.click(await screen.findByText('Onboarding for new engineers'));
    const panel = await screen.findByRole('region', { name: 'Onboarding for new engineers' });

    expect(await within(panel).findByText('meeting-notes')).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Unlink meeting-notes' })).toBeInTheDocument();
    expect(document.body.innerHTML).not.toContain('/home/ada');
    expect(document.querySelector('[title*="/home/ada"]')).toBeNull();
  });

  it('keeps an import to the team database from starting while it links to such a folder, and says why', async () => {
    const { invoke } = fakeKnowledge('ok', true, routedRemote('remoteLocal', 'remoteLocal'));
    await show();

    await fireEvent.click(screen.getAllByRole('button', { name: 'Add documents' })[0]);
    await fireEvent.click(await screen.findByRole('button', { name: 'Choose files' }));
    await screen.findByText('5 files');

    const group = await screen.findByRole('radiogroup', { name: 'Save to' });
    await fireEvent.click(within(group).getByRole('radio', { name: 'This computer' }));

    await fireEvent.click(screen.getByRole('button', { name: 'Link a project' }));
    await fireEvent.click(await screen.findByRole('menuitem', { name: new RegExp(FOLDER) }));
    expect(screen.getByRole('button', { name: 'Start import' })).toBeEnabled();

    await fireEvent.click(within(group).getByRole('radio', { name: 'Remote' }));

    const start = screen.getByRole('button', { name: 'Start import' });
    expect(start).toBeDisabled();
    expect(start).toHaveAccessibleDescription('Unlink the folders without a git remote to import');
    expect(screen.getByText(REASON)).toBeInTheDocument();
    expect(calls(invoke, 'knowledge_import')).toHaveLength(0);
  });
});
