import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls, fakeHooks, pickTarget, PROJECT, registerHooksHooks, rowOf, showHooks } from './hooks-test-support';

registerHooksHooks();

async function openEditor() {
  await showHooks();
  await fireEvent.click(screen.getAllByRole('button', { name: 'New hook' })[0]);

  return screen.findByRole('dialog', { name: 'New hook' });
}

async function chooseEvent(dialog: HTMLElement, name: RegExp) {
  await fireEvent.click(within(dialog).getByRole('button', { name: 'When it fires' }));
  await fireEvent.click(await screen.findByRole('menuitemradio', { name }));
}

const actionsOf = (dialog: HTMLElement) => within(within(dialog).getByRole('radiogroup', { name: 'What it does' })).getAllByRole('radio').map((radio) => radio.textContent?.trim());

describe('Hook editor', () => {
  it('opens on a fresh hook that cannot be saved until it has a name and a text', async () => {
    fakeHooks();
    const dialog = await openEditor();

    expect(within(dialog).getByRole('button', { name: 'Add hook' })).toBeDisabled();
    expect(within(dialog).getByRole('button', { name: 'When it fires' })).toHaveTextContent('Session start');

    // Nothing is said about a field the user has not been at yet.
    expect(within(dialog).queryByRole('alert')).not.toBeInTheDocument();

    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Mind the budget' } });
    await fireEvent.input(within(dialog).getByLabelText('Context'), { target: { value: 'Check the limits first.' } });

    expect(within(dialog).getByRole('button', { name: 'Add hook' })).toBeEnabled();
  });

  it.each([
    [/Session start/, ['Add context', 'Command']],
    [/Prompt submit/, ['Add context', 'Command']],
    [/After tool call/, ['Follow-up', 'Command']],
    [/Turn end/, ['Follow-up', 'Command']],
    [/Session end/, ['Command']],
  ])('offers only the actions that fit the event %s', async (event, actions) => {
    fakeHooks();
    const dialog = await openEditor();

    await chooseEvent(dialog, event);

    expect(actionsOf(dialog)).toEqual(actions);
  });

  it('switches the action to the first one the new event allows, and shows the field of the action', async () => {
    fakeHooks();
    const dialog = await openEditor();
    expect(within(dialog).getByLabelText('Context')).toBeInTheDocument();

    await chooseEvent(dialog, /Turn end/);
    expect(within(dialog).getByLabelText('Message')).toBeInTheDocument();
    expect(within(dialog).queryByLabelText('Context')).not.toBeInTheDocument();

    await chooseEvent(dialog, /Session end/);
    expect(within(dialog).getByLabelText('Command')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('Timeout')).toBeInTheDocument();
  });

  it('tells the author of a command to quote the values that come from the agent', async () => {
    fakeHooks();
    const dialog = await openEditor();
    const advice = /come from the agent\. Quote them/;

    expect(within(dialog).queryByText(advice)).not.toBeInTheDocument();

    await chooseEvent(dialog, /Session end/);
    expect(within(dialog).getByText(advice)).toHaveTextContent('"$HARNESS_HOOK_TOUCHED"');
  });

  it('asks for a tool filter at the tool event only, and checks it while it is typed', async () => {
    fakeHooks();
    const dialog = await openEditor();
    expect(within(dialog).queryByLabelText('Tool filter')).not.toBeInTheDocument();

    await chooseEvent(dialog, /After tool call/);
    const filter = await within(dialog).findByLabelText('Tool filter');
    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Lint' } });
    await fireEvent.input(within(dialog).getByLabelText('Message'), { target: { value: 'Run the linter.' } });
    expect(within(dialog).getByRole('button', { name: 'Add hook' })).toBeEnabled();

    await fireEvent.input(filter, { target: { value: 'edit | write' } });
    expect(await within(dialog).findByText('Tool names use letters, digits and _ . : - only, with no spaces.')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Add hook' })).toBeDisabled();

    await fireEvent.input(filter, { target: { value: 'edit||write' } });
    expect(await within(dialog).findByText('There is an empty name between the bars.')).toBeInTheDocument();

    await fireEvent.input(filter, { target: { value: 'edit|write' } });
    await waitFor(() => expect(within(dialog).queryByRole('alert')).not.toBeInTheDocument());
    expect(within(dialog).getByRole('button', { name: 'Add hook' })).toBeEnabled();
  });

  it('checks the timeout of a command', async () => {
    fakeHooks();
    const dialog = await openEditor();
    await chooseEvent(dialog, /Session end/);
    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Clean up' } });
    await fireEvent.input(within(dialog).getByLabelText('Command'), { target: { value: 'rm -rf .cache' } });

    await fireEvent.input(within(dialog).getByLabelText('Timeout'), { target: { value: '45' } });
    expect(await within(dialog).findByText('Use a whole number from 1 to 30.')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Add hook' })).toBeDisabled();

    await fireEvent.input(within(dialog).getByLabelText('Timeout'), { target: { value: '30' } });
    await waitFor(() => expect(within(dialog).getByRole('button', { name: 'Add hook' })).toBeEnabled());
  });

  it('writes the hook, closes, and shows it in its scope with its panel open', async () => {
    const { invoke } = fakeHooks();
    const dialog = await openEditor();

    await chooseEvent(dialog, /After tool call/);
    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: ' Lint ' } });
    await fireEvent.input(within(dialog).getByLabelText('Tool filter'), { target: { value: 'edit|write' } });
    await fireEvent.click(within(dialog).getByRole('radio', { name: 'Command' }));
    await fireEvent.input(within(dialog).getByLabelText('Command'), { target: { value: 'bun run lint' } });
    await pickTarget(dialog, /^demo/);
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add hook' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New hook' })).not.toBeInTheDocument());
    expect(calls(invoke, 'hook_save')).toEqual([
      {
        draft: {
          id: null,
          expectedSha256: null,
          name: 'Lint',
          description: null,
          scope: 'project',
          project: PROJECT,
          event: 'toolEnd',
          matcher: 'edit|write',
          action: { type: 'command', command: 'bun run lint', timeoutSecs: null },
          enabled: true,
        },
      },
    ]);
    expect(within(await screen.findByRole('region', { name: 'Lint' })).getByText('bun run lint')).toBeInTheDocument();
    expect(within(rowOf('Lint')).getByText('Active')).toBeInTheDocument();
    expect(toasts.items.map((toast) => [toast.title, toast.description])).toContainEqual(['Lint added', 'It runs after the agent uses a tool.']);
  });

  it('edits a hook that exists, naming the version it started from', async () => {
    const { invoke } = fakeHooks();
    await showHooks();
    await fireEvent.click(rowOf('Remind about tests'));
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Remind about tests' })).getByRole('button', { name: 'Edit' }));

    const dialog = await screen.findByRole('dialog', { name: 'Edit hook' });
    expect(within(dialog).getByLabelText('Name')).toHaveValue('Remind about tests');
    expect(within(dialog).getByLabelText('Context')).toHaveValue('Run the test suite before you say you are done, and add a test for any behaviour you change.');

    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Remind about the tests' } });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(within(rowOf('Remind about the tests')).getByText('Off')).toBeInTheDocument());
    expect(calls(invoke, 'hook_save')).toEqual([expect.objectContaining({ draft: expect.objectContaining({ id: expect.stringMatching(/^[0-9a-f]{32}$/), expectedSha256: expect.stringMatching(/^[0-9a-f]{64}$/), name: 'Remind about the tests', enabled: false }) })]);
  });

  it('stays open and says so when the hook changed meanwhile', async () => {
    const { state } = fakeHooks();
    await showHooks();
    await fireEvent.click(rowOf('Remind about tests'));
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Remind about tests' })).getByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit hook' });

    state.agentChanges(viewState.hookId ?? '');
    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Other' } });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    expect(await within(dialog).findByText(/This hook changed in the meantime/)).toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: 'Edit hook' })).toBeInTheDocument();
  });

  it('deletes a hook only after the user confirms', async () => {
    const { invoke } = fakeHooks();
    await showHooks();
    await fireEvent.click(rowOf('Remind about tests'));
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Remind about tests' })).getByRole('button', { name: 'Delete' }));

    const confirm = await screen.findByRole('dialog', { name: 'Delete this hook?' });
    await fireEvent.click(within(confirm).getByRole('button', { name: 'Keep it' }));
    expect(calls(invoke, 'hook_delete')).toHaveLength(0);

    await fireEvent.click(within(screen.getByRole('region', { name: 'Remind about tests' })).getByRole('button', { name: 'Delete' }));
    await fireEvent.click(within(await screen.findByRole('dialog', { name: 'Delete this hook?' })).getByRole('button', { name: 'Delete hook' }));

    await waitFor(() => expect(screen.queryByText('Remind about tests', { selector: '[data-row] td div' })).not.toBeInTheDocument());
    expect(calls(invoke, 'hook_delete')).toHaveLength(1);
    expect(viewState.hookId).toBeNull();
  });
});
