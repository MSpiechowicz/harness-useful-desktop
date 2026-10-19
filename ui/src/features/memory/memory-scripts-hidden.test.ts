import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { sampleScripts } from '$lib/ipc/mock/memory-scripts-fixtures';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls, fakeMemory, PROJECT, registerMemoryHooks, rowButton, showMemory } from './memory-test-support';

registerMemoryHooks();

const openScript = async (name: string) => {
  await fireEvent.click(rowButton(name) as HTMLElement);

  return screen.findByRole('region', { name });
};

/** Remote scripts the way a routing that saves on this computer shows them: read-only. */
const readOnlyRemote = () => sampleScripts().map((script) => ({ ...script, readOnly: script.place === 'team' }));

describe('hiding remote scripts on this computer', () => {
  it('lets a read-only remote script be hidden and nothing else', async () => {
    const invoke = fakeMemory({ kind: 'available' }, readOnlyRemote());
    await showMemory();

    const panel = await openScript('open-prs');

    expect(within(panel).queryByRole('button', { name: 'Edit' })).toBeNull();
    expect(within(panel).queryByRole('button', { name: 'Delete' })).toBeNull();
    expect(within(panel).getByText(/This remote script is read-only here/)).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Hide on this computer' }));

    await waitFor(() => expect(calls(invoke, 'memory_script_hide')).toEqual([{ id: 12, project: PROJECT, hidden: true }]));
    expect(await screen.findByRole('button', { name: 'Hidden on this computer (1)' })).toBeInTheDocument();
    expect(rowButton('open-prs')).toBeNull();
    expect(toasts.items.at(-1)).toMatchObject({ title: 'Hidden on this computer', description: 'open-prs', tone: 'success' });
  });

  it('keeps Edit and Delete next to Hide for a remote script of a routing on the remote database, and offers no Hide for a script of this computer', async () => {
    fakeMemory();
    await showMemory();

    const remote = await openScript('open-prs');

    await waitFor(() => expect(within(remote).getByRole('button', { name: 'Edit' })).toBeEnabled());
    expect(within(remote).getByRole('button', { name: 'Delete' })).toBeInTheDocument();
    expect(within(remote).getByRole('button', { name: 'Hide on this computer' })).toBeInTheDocument();

    const local = await openScript('ci-status');

    expect(within(local).getByRole('button', { name: 'Delete' })).toBeInTheDocument();
    expect(within(local).queryByRole('button', { name: 'Hide on this computer' })).toBeNull();
  });

  it('shows the hidden scripts in their own filter, where they can only be shown again', async () => {
    const scripts = sampleScripts().map((script) => (script.name === 'disk-hogs' ? { ...script, hidden: true } : script));
    const invoke = fakeMemory({ kind: 'available' }, scripts);
    await showMemory();

    expect(rowButton('disk-hogs')).toBeNull();

    await fireEvent.click(await screen.findByRole('button', { name: 'Hidden on this computer (1)' }));

    await waitFor(() => expect(calls(invoke, 'memory_scripts_list').at(-1)).toMatchObject({ hidden: true, offset: 0 }));
    expect(await screen.findByText('1 script')).toBeInTheDocument();

    const panel = await openScript('disk-hogs');

    expect(calls(invoke, 'memory_script_get').at(-1)).toMatchObject({ id: 21, hidden: true });
    expect(within(panel).queryByRole('button', { name: 'Edit' })).toBeNull();
    expect(within(panel).queryByRole('button', { name: 'Delete' })).toBeNull();
    // Agents do not see it, so it asks for no approval either.
    expect(within(panel).queryByRole('button', { name: 'Trust' })).toBeNull();
    expect(within(panel).getByText(/This script is hidden on this computer/)).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Unhide' }));

    await waitFor(() => expect(calls(invoke, 'memory_script_hide')).toEqual([{ id: 21, project: PROJECT, hidden: false }]));
    // The last hidden script is shown again: the filter has nothing left and the list is the full one.
    await waitFor(() => expect(screen.queryByRole('button', { name: /Hidden on this computer/ })).toBeNull());
    expect(await screen.findByText('5 scripts')).toBeInTheDocument();
    expect(rowButton('disk-hogs')).not.toBeNull();
  });

  it('says so when a script cannot be hidden', async () => {
    const invoke = fakeMemory({ kind: 'available' }, readOnlyRemote());
    await showMemory();
    const panel = await openScript('open-prs');
    const real = invoke.getMockImplementation() as (name: string, args: unknown) => Promise<unknown>;
    invoke.mockImplementation((async (name: string, args: unknown) => {
      if (name === 'memory_script_hide') {
        throw new Error('the remote database did not answer');
      }

      return real(name, args);
    }) as never);

    await fireEvent.click(within(panel).getByRole('button', { name: 'Hide on this computer' }));

    await waitFor(() => expect(toasts.items.at(-1)).toMatchObject({ title: 'Could not hide the script', tone: 'danger' }));
    expect(rowButton('open-prs')).not.toBeNull();
  });
});
