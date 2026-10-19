import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { harness } from '$lib/stores/harness.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls, fakeRoutines, registerRoutinesHooks, rowOf, showRoutines } from './routines-test-support';

registerRoutinesHooks();

/** Opens the editor on a new routine from the primary action of the header. */
async function openEditor(): Promise<HTMLElement> {
  await showRoutines();
  await fireEvent.click(screen.getAllByRole('button', { name: 'New routine' })[0]);

  return screen.findByRole('dialog', { name: 'New routine' });
}

/** Opens one of the menus of the editor by its label and picks a choice. */
async function choose(dialog: HTMLElement, label: string, choice: RegExp): Promise<void> {
  await fireEvent.click(await within(dialog).findByRole('button', { name: label }));
  await fireEvent.click(await screen.findByRole('menuitemradio', { name: choice }));
}

describe('The New routine dialog', () => {
  it('asks for the name first, and does not add anything until the routine is complete', async () => {
    fakeRoutines();
    const dialog = await openEditor();

    expect(within(dialog).getByLabelText('Name')).toHaveFocus();
    expect(within(dialog).getByRole('button', { name: 'Add routine' })).toBeDisabled();

    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Archive logs' } });
    expect(within(dialog).getByRole('button', { name: 'Add routine' })).toBeDisabled();

    await fireEvent.input(within(dialog).getByLabelText('Prompt'), { target: { value: 'Pack up last week\'s logs.' } });
    expect(within(dialog).getByRole('button', { name: 'Add routine' })).toBeEnabled();
  });

  it('adds a command routine at a time of the day, and lists it', async () => {
    const { invoke } = fakeRoutines();
    const dialog = await openEditor();

    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Archive logs' } });
    await fireEvent.click(within(dialog).getByRole('radio', { name: 'Command' }));
    await fireEvent.input(within(dialog).getByLabelText('Command'), { target: { value: 'tar czf logs.tgz logs' } });
    await fireEvent.input(within(dialog).getByLabelText('Time'), { target: { value: '23:15' } });

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add routine' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New routine' })).not.toBeInTheDocument());
    expect(calls(invoke, 'routine_save')).toEqual([
      {
        draft: {
          id: null,
          expectedSha256: null,
          name: 'Archive logs',
          description: null,
          scope: 'global',
          project: null,
          when: { type: 'daily', at: '23:15' },
          action: { type: 'command', command: 'tar czf logs.tgz logs', timeoutSecs: null },
          enabled: true,
        },
      },
    ]);
    expect(within(rowOf('Archive logs')).getByText('Every day at 23:15')).toBeInTheDocument();
    expect(toasts.items.map((toast) => [toast.title, toast.description])).toContainEqual(['Archive logs added', 'Schedule: Every day at 23:15.']);
  });

  it('previews the next runs of the schedule, and says what is wrong with one that never runs', async () => {
    fakeRoutines();
    const dialog = await openEditor();

    expect(await within(dialog).findByText('Next runs:')).toBeInTheDocument();

    await choose(dialog, 'When it runs', /^Cron/);
    await fireEvent.input(within(dialog).getByLabelText('Cron expression'), { target: { value: '0 0 31 2 *' } });

    expect(await within(dialog).findByText('This schedule never runs.')).toBeInTheDocument();
    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Never' } });
    await fireEvent.input(within(dialog).getByLabelText('Prompt'), { target: { value: 'Never happens.' } });
    expect(within(dialog).getByRole('button', { name: 'Add routine' })).toBeDisabled();

    await fireEvent.input(within(dialog).getByLabelText('Cron expression'), { target: { value: '*/30 9-17 * * mon-fri' } });

    await waitFor(() => expect(within(dialog).queryByText('This schedule never runs.')).not.toBeInTheDocument());
    expect(await within(dialog).findByText('Next runs:')).toBeInTheDocument();
    await waitFor(() => expect(within(dialog).getByRole('button', { name: 'Add routine' })).toBeEnabled());
  });

  it('counts every N minutes and hours, and refuses a number out of range', async () => {
    fakeRoutines();
    const dialog = await openEditor();

    await choose(dialog, 'When it runs', /^Every N minutes/);
    await fireEvent.input(within(dialog).getByLabelText('Every'), { target: { value: '900' } });

    expect(within(dialog).getByText('Use a whole number from 1 to 720.')).toBeInTheDocument();

    await choose(dialog, 'When it runs', /^Every N hours/);

    expect(within(dialog).getByText('Use a whole number from 1 to 24.')).toBeInTheDocument();
  });

  it('warns about a routine that runs tools without asking, and sends the approval mode with OMP', async () => {
    const { invoke } = fakeRoutines();
    const dialog = await openEditor();

    expect(within(dialog).getByRole('button', { name: 'Approval mode' })).toBeInTheDocument();
    expect(within(dialog).queryByRole('switch', { name: 'Trust the project once' })).not.toBeInTheDocument();
    expect(within(dialog).queryByText('Runs tools without asking')).not.toBeInTheDocument();

    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Unattended' } });
    await fireEvent.input(within(dialog).getByLabelText('Prompt'), { target: { value: 'Tidy up.' } });
    await choose(dialog, 'Approval mode', /^Yolo/);
    expect(await within(dialog).findByText('Runs tools without asking')).toBeInTheDocument();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add routine' }));

    await waitFor(() => expect(calls(invoke, 'routine_save')).toHaveLength(1));
    expect(calls(invoke, 'routine_save')[0]).toMatchObject({ draft: { action: { harness: 'omp', approvalMode: 'yolo', trustOnce: false } } });
  });

  it('does not offer a harness that is not installed on this computer', async () => {
    fakeRoutines();
    const dialog = await openEditor();

    expect(within(dialog).getByRole('radio', { name: 'OMP' })).toBeEnabled();
    expect(within(dialog).getByRole('radio', { name: 'PI' })).toBeDisabled();
  });

  it('drops the approval mode when the routine moves to PI, which trusts the project once instead', async () => {
    const { invoke } = fakeRoutines();
    const installed = harness.summaries;
    harness.summaries = installed.map((summary) => (summary.info.id === 'pi' ? { ...summary, detection: { ...summary.detection, installed: true } } : summary));
    const dialog = await openEditor();

    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'On PI' } });
    await fireEvent.input(within(dialog).getByLabelText('Prompt'), { target: { value: 'Tidy up.' } });
    await choose(dialog, 'Approval mode', /^Write/);
    await fireEvent.click(within(dialog).getByRole('radio', { name: 'PI' }));

    await fireEvent.click(await within(dialog).findByRole('switch', { name: 'Trust the project once' }));
    expect(await within(dialog).findByText('Runs tools without asking')).toBeInTheDocument();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add routine' }));

    await waitFor(() => expect(calls(invoke, 'routine_save')).toHaveLength(1));
    expect(calls(invoke, 'routine_save')[0]).toMatchObject({ draft: { action: { harness: 'pi', approvalMode: null, trustOnce: true } } });
    harness.summaries = installed;
  });

  it('authors an OpenCode routine with an approval mode and without the project trust, since OpenCode has the one and not the other', async () => {
    const { invoke } = fakeRoutines();
    const dialog = await openEditor();

    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'On OpenCode' } });
    await fireEvent.input(within(dialog).getByLabelText('Prompt'), { target: { value: 'Tidy up.' } });
    await fireEvent.click(within(dialog).getByRole('radio', { name: 'OpenCode' }));

    await choose(dialog, 'Approval mode', /^Write/);
    expect(within(dialog).queryByRole('switch', { name: 'Trust the project once' })).not.toBeInTheDocument();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add routine' }));

    await waitFor(() => expect(calls(invoke, 'routine_save')).toHaveLength(1));
    expect(calls(invoke, 'routine_save')[0]).toMatchObject({ draft: { action: { harness: 'opencode', approvalMode: 'write', trustOnce: false } } });
  });

  it('offers only the harnesses that can chat', async () => {
    fakeRoutines();
    const listed = harness.summaries;
    harness.summaries = listed.map((summary) => (summary.info.id === 'opencode' ? { ...summary, capabilities: { ...summary.capabilities, chat: false } } : summary));
    const dialog = await openEditor();

    expect(within(dialog).getByRole('radio', { name: 'OMP' })).toBeInTheDocument();
    expect(within(dialog).queryByRole('radio', { name: 'OpenCode' })).not.toBeInTheDocument();
    harness.summaries = listed;
  });

  it('sends the model only when both parts are given', async () => {
    const { invoke } = fakeRoutines();
    const dialog = await openEditor();

    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Pick a model' } });
    await fireEvent.input(within(dialog).getByLabelText('Prompt'), { target: { value: 'Say hello.' } });
    await fireEvent.input(within(dialog).getByLabelText('Provider'), { target: { value: 'anthropic' } });

    expect(within(dialog).getByText('Fill in both the provider and the model id, or neither.')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Add routine' })).toBeDisabled();

    await fireEvent.input(within(dialog).getByLabelText('Model id'), { target: { value: 'claude-sonnet-4-5' } });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add routine' }));

    await waitFor(() => expect(calls(invoke, 'routine_save')).toHaveLength(1));
    expect(calls(invoke, 'routine_save')[0]).toMatchObject({ draft: { action: { type: 'agent', harness: 'omp', model: { provider: 'anthropic', id: 'claude-sonnet-4-5' }, prompt: 'Say hello.', approvalMode: null, trustOnce: false } } });
  });

  it('offers the scripts saved in Harness Memory, and runs the one the user picks', async () => {
    const { invoke } = fakeRoutines();
    const dialog = await openEditor();

    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Look for hogs' } });
    await fireEvent.click(within(dialog).getByRole('radio', { name: 'Script' }));
    await choose(dialog, 'Script', /^disk-hogs/);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add routine' }));

    await waitFor(() => expect(calls(invoke, 'routine_save')).toHaveLength(1));
    expect(calls(invoke, 'routine_save')[0]).toMatchObject({ draft: { action: { type: 'script', name: 'disk-hogs', scope: 'global' } } });
  });

  it('edits a routine in place, naming the version it started from', async () => {
    const { invoke } = fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf('Check the disk'));
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Check the disk' })).getByRole('button', { name: 'Edit' }));

    const dialog = await screen.findByRole('dialog', { name: 'Edit routine' });

    expect(within(dialog).getByLabelText('Name')).toHaveValue('Check the disk');
    expect(within(dialog).getByLabelText('Command')).toHaveValue('du -sh ~/.cache ~/.local/share | sort -h');
    expect(within(dialog).getByLabelText('Time')).toHaveValue('03:30');

    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Check the disks' } });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(within(rowOf('Check the disks')).getByText('Every day at 03:30')).toBeInTheDocument());
    expect(calls(invoke, 'routine_save')[0]).toMatchObject({ draft: { id: expect.stringMatching(/^[0-9a-f]{32}$/), expectedSha256: expect.stringMatching(/^[0-9a-f]{64}$/), name: 'Check the disks' } });
  });

  it('says that the routine changed meanwhile, instead of overwriting the change', async () => {
    const { state } = fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf('Check the disk'));
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Check the disk' })).getByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit routine' });

    // An agent rewrites the file while the dialog is open: its bytes, and so the version the dialog started from, are gone.
    const view = state.list(null).routines.find((candidate) => candidate.name === 'Check the disk');
    state.rewrite(view?.id ?? '', { description: 'Touched elsewhere' });

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    expect(await within(dialog).findByText(/This routine changed in the meantime\./)).toBeInTheDocument();
  });
});
