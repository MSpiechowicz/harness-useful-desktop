import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { IpcFailure } from '$lib/ipc/transport';

import { calls, fakeRoutines, fakeRoutinesWhere, registerRoutinesHooks, rowOf, showRoutines } from './routines-test-support';

registerRoutinesHooks();

async function openEditor(): Promise<HTMLElement> {
  await showRoutines();
  await fireEvent.click(screen.getAllByRole('button', { name: 'New routine' })[0]);

  const dialog = await screen.findByRole('dialog', { name: 'New routine' });
  await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Pick a model' } });
  await fireEvent.input(within(dialog).getByLabelText('Prompt'), { target: { value: 'Say hello.' } });

  return dialog;
}

/** Opens the menu of a field of the editor. */
async function open(dialog: HTMLElement, field: string): Promise<void> {
  await fireEvent.click(await within(dialog).findByRole('button', { name: field }));
}

/** The names of the choices of the open menu, top to bottom. */
function choices(): string[] {
  return screen.getAllByRole('menuitemradio').map((item) => item.querySelector('span.flex-1')?.textContent?.trim() ?? '');
}

async function choose(choice: RegExp): Promise<void> {
  await fireEvent.click(await screen.findByRole('menuitemradio', { name: choice }));
}

/** The helper agents the editor started so far, and whether all of them were stopped again. */
function helpers(invoke: Parameters<typeof calls>[0]): { started: number; allStopped: boolean } {
  const started = calls(invoke, 'agent_start').length;

  return { started, allStopped: calls(invoke, 'agent_stop').length >= started };
}

describe('The model of an agent routine', () => {
  it('is chosen from the models of the harness, with the harness default first, and asked once', async () => {
    const { invoke } = fakeRoutines();
    const dialog = await openEditor();

    expect(within(dialog).getByRole('button', { name: 'Model' })).toHaveTextContent('The harness default');
    expect(within(dialog).queryByRole('button', { name: 'Thinking' })).not.toBeInTheDocument();

    await open(dialog, 'Model');
    await screen.findByRole('menuitemradio', { name: /GPT-5/ });
    expect(choices()).toEqual(['The harness default', 'Claude Sonnet 4.5', 'Claude Haiku 4.5', 'GPT-5', 'qwen3-coder:30b']);
    expect(screen.getByRole('menuitemradio', { name: /The harness default/ })).toBeChecked();

    // The helper is of the routine's harness, a utility session that is stopped again.
    expect(calls(invoke, 'agent_start')).toMatchObject([{ request: { harness: 'omp', utility: true, resume: null } }]);

    await open(dialog, 'Model');
    await open(dialog, 'Model');
    await screen.findByRole('menuitemradio', { name: /GPT-5/ });
    await waitFor(() => expect(helpers(invoke)).toEqual({ started: 1, allStopped: true }));

    await choose(/GPT-5/);
    expect(within(dialog).getByRole('button', { name: 'Model' })).toHaveTextContent('GPT-5');
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add routine' }));

    await waitFor(() => expect(calls(invoke, 'routine_save')).toHaveLength(1));
    expect(calls(invoke, 'routine_save')[0]).toMatchObject({ draft: { action: { type: 'agent', model: { provider: 'openai', id: 'gpt-5' } } } });
    expect(((calls(invoke, 'routine_save')[0] as { draft: { action: { model: object } } }).draft.action.model)).not.toHaveProperty('thinking');
  });

  it('can go back to the harness default', async () => {
    const { invoke } = fakeRoutines();
    const dialog = await openEditor();

    await open(dialog, 'Model');
    await choose(/Claude Haiku/);
    await open(dialog, 'Model');
    await choose(/The harness default/);
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add routine' }));

    await waitFor(() => expect(calls(invoke, 'routine_save')).toHaveLength(1));
    expect(calls(invoke, 'routine_save')[0]).toMatchObject({ draft: { action: { model: null } } });
  });

  it('says so when the harness cannot be asked, and still lets the default be chosen', async () => {
    fakeRoutinesWhere((view) => view, {
      agent_start: () => {
        throw IpcFailure.from({ code: 'untrusted', message: 'Claude Code does not trust this folder yet.' });
      },
    });
    const dialog = await openEditor();

    await open(dialog, 'Model');
    expect(await screen.findByText(/The models of this harness could not be listed/)).toBeInTheDocument();
    await choose(/The harness default/);

    expect(within(dialog).getByRole('button', { name: 'Model' })).toHaveTextContent('The harness default');
  });
});

describe('The thinking level of an agent routine', () => {
  it('is chosen after the model from the levels the model offers, and goes with the model it was chosen for', async () => {
    const { invoke } = fakeRoutines();
    const dialog = await openEditor();

    await open(dialog, 'Model');
    await choose(/GPT-5/);

    await open(dialog, 'Thinking');
    await screen.findByRole('menuitemradio', { name: 'high' });
    expect(choices()).toEqual(['Model default', 'off', 'minimal', 'low', 'medium', 'high']);
    await choose(/^high$/);
    expect(within(dialog).getByRole('button', { name: 'Thinking' })).toHaveTextContent('high');

    // The helper that listed the levels switched to the model first, and was stopped.
    const sent = calls(invoke, 'agent_send').map((call) => (call as { action: { type: string } }).action.type);
    expect(sent).toEqual(['getAvailableModels', 'setModel', 'getAvailableThinkingLevels']);
    await waitFor(() => expect(helpers(invoke)).toEqual({ started: 2, allStopped: true }));

    await open(dialog, 'Model');
    await choose(/Claude Haiku/);
    expect(await within(dialog).findByRole('button', { name: 'Thinking' })).toHaveTextContent('Model default');

    await open(dialog, 'Thinking');
    await choose(/^low$/);
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add routine' }));

    await waitFor(() => expect(calls(invoke, 'routine_save')).toHaveLength(1));
    expect(calls(invoke, 'routine_save')[0]).toMatchObject({ draft: { action: { model: { provider: 'anthropic', id: 'claude-haiku-4-5', thinking: 'low' } } } });
  });

  it('is not offered for the harness default, or for a model that does not reason', async () => {
    fakeRoutines();
    const dialog = await openEditor();

    await open(dialog, 'Model');
    await choose(/qwen3-coder/);

    expect(within(dialog).queryByRole('button', { name: 'Thinking' })).not.toBeInTheDocument();

    await open(dialog, 'Model');
    await choose(/GPT-5/);
    expect(await within(dialog).findByRole('button', { name: 'Thinking' })).toBeInTheDocument();

    await open(dialog, 'Model');
    await choose(/The harness default/);
    await waitFor(() => expect(within(dialog).queryByRole('button', { name: 'Thinking' })).not.toBeInTheDocument());
  });
});

describe('A routine saved with a model the harness does not list', () => {
  it('shows it as its id, with its thinking level, and saves it as it was', async () => {
    const saved = { provider: 'acme', id: 'gizmo-1', thinking: 'deep' };
    const { invoke } = fakeRoutinesWhere((view) => {
      const routine = view.routine;

      return routine?.name === 'Triage new issues' && routine.action.type === 'agent' ? { ...view, routine: { ...routine, action: { ...routine.action, model: saved } } } : view;
    });

    await showRoutines();
    await fireEvent.click(rowOf('Triage new issues'));
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Triage new issues' })).getByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit routine' });

    expect(within(dialog).getByRole('button', { name: 'Model' })).toHaveTextContent('gizmo-1');
    expect(within(dialog).getByRole('button', { name: 'Thinking' })).toHaveTextContent('deep');

    await open(dialog, 'Model');
    await screen.findByRole('menuitemradio', { name: /GPT-5/ });
    expect(choices()).toEqual(['The harness default', 'gizmo-1', 'Claude Sonnet 4.5', 'Claude Haiku 4.5', 'GPT-5', 'qwen3-coder:30b']);
    expect(screen.getByRole('menuitemradio', { name: 'gizmo-1' })).toBeChecked();
    await open(dialog, 'Model');

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'routine_save')).toHaveLength(1));
    expect(calls(invoke, 'routine_save')[0]).toMatchObject({ draft: { action: { model: saved } } });
  });
});
