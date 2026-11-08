import { fireEvent, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { noteList } from '$lib/stores/memoryNotes.svelte';
import { scriptList } from '$lib/stores/memoryScripts.svelte';

import { outageOf, scriptsWithLocal } from './memory-sync-support';
import { fakeMemory, registerMemoryHooks, showMemory, showNotes, withTeamDatabase } from './memory-test-support';
import { notesTeam, scriptsTeam } from './notes-team.svelte';

registerMemoryHooks();

const KINDS = [
  {
    name: 'notes',
    team: notesTeam,
    banner: () => screen.findByText(`${sampleNotes().filter((note) => note.place === 'local').length} notes saved on this computer are not in the remote database.`),
    stranded: sampleNotes().filter((note) => note.place === 'local').length,
    open: async () => {
      const invoke = fakeMemory();
      withTeamDatabase(invoke, { repositoryNotes: 'remote', globalNotes: 'remote' });
      const outage = outageOf(invoke);
      await showNotes();

      return { outage, reread: async () => noteList.refresh(), isOffline: () => noteList.data?.offline === true };
    },
  },
  {
    name: 'scripts',
    team: scriptsTeam,
    banner: () => screen.findByText('2 scripts saved on this computer are not in the remote database.'),
    stranded: 2,
    open: async () => {
      const invoke = fakeMemory({ kind: 'available' }, scriptsWithLocal());
      withTeamDatabase(invoke, { repositoryNotes: 'local', globalNotes: 'local', scripts: 'remote' });
      const outage = outageOf(invoke);
      await showMemory();

      return { outage, reread: async () => scriptList.refresh(), isOffline: () => scriptList.data?.offline === true };
    },
  },
];

const bannerGone = () => screen.queryByText(/saved on this computer (is|are) not in the remote database/);

describe.each(KINDS)('the banner for $name saved on this computer during an outage of the remote database', ({ team, banner, stranded, open }) => {
  it('is hidden while the remote database is down, because Copy fails and deleting is not safe, and returns when it is back', async () => {
    const { outage, reread, isOffline } = await open();
    await banner();

    outage.down = true;
    await reread();

    await waitFor(() => expect(isOffline()).toBe(true));
    await waitFor(() => expect(bannerGone()).toBeNull());
    expect(screen.queryByRole('button', { name: 'Copy…' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Delete from this computer…' })).toBeNull();

    outage.down = false;
    await reread();

    await waitFor(() => expect(isOffline()).toBe(false));
    await banner();
    expect(screen.getByRole('button', { name: 'Copy…' })).toBeInTheDocument();
  });

  it('leaves the count it was dismissed at alone, so an outage does not bring it back or forget the dismissal', async () => {
    const { outage, reread, isOffline } = await open();
    await banner();

    await fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
    await waitFor(() => expect(bannerGone()).toBeNull());
    expect(team.dismissedAt).toBe(stranded);

    outage.counts = { repo: 0, global: 0 };
    outage.down = true;
    await reread();

    await waitFor(() => expect(isOffline()).toBe(true));
    expect(bannerGone()).toBeNull();
    expect(team.dismissedAt).toBe(stranded);

    outage.down = false;
    await reread();

    await waitFor(() => expect(isOffline()).toBe(false));
    expect(bannerGone()).toBeNull();
    expect(team.dismissedAt).toBe(stranded);
  });
});
