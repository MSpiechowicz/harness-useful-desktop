import { fireEvent, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { viewState } from '$lib/stores/view-state.svelte';

import { appliesTo, calls, EMPTY_PROJECT, fakeMemory, pickTarget, registerMemoryHooks } from './memory-test-support';
import { BODY, fill, openEditor } from './script-editor-support';

registerMemoryHooks();

describe('where a new script applies', () => {
  it('keeps a script global when it is chosen, and saves it with no project', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    expect(within(dialog).getByText('Applies to')).toBeInTheDocument();
    expect(appliesTo(dialog)).toHaveTextContent('demo');

    await pickTarget(dialog, 'Global');
    expect(within(dialog).getByText('For every project.')).toBeInTheDocument();

    await fill(dialog, { name: 'list-todos', description: 'Lists the todos.', body: BODY });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add script' }));

    await waitFor(() => expect(calls(invoke, 'memory_script_save')).toHaveLength(1));
    expect(calls(invoke, 'memory_script_save')[0]).toMatchObject({ project: null, draft: { scope: 'global' } });
  });

  it('saves the script in a recent project that is not the open one, and names that project in the help', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await pickTarget(dialog, /^research/);

    expect(appliesTo(dialog)).toHaveTextContent('research');
    expect(within(dialog).getByText('Only for research.')).toBeInTheDocument();

    await fill(dialog, { name: 'list-todos', description: 'Lists the todos.', body: BODY });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add script' }));

    await waitFor(() => expect(calls(invoke, 'memory_script_save')).toHaveLength(1));
    expect(calls(invoke, 'memory_script_save')[0]).toMatchObject({ project: EMPTY_PROJECT, draft: { scope: 'repo' } });
  });

  it('starts on what the Scope filter looks at', async () => {
    viewState.memoryReach = 'global';
    fakeMemory();
    const dialog = await openEditor();

    expect(appliesTo(dialog)).toHaveTextContent('Global');
  });

  it('drops the finding once Applies to moves to another project, so Replace never reaches a script of the first one', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { name: 'ci-status', description: 'A newer one.', body: BODY });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add script' }));
    await within(dialog).findByText('This name is taken');

    await pickTarget(dialog, /^research/);

    expect(within(dialog).queryByText('This name is taken')).not.toBeInTheDocument();
    expect(within(dialog).queryByRole('button', { name: 'Replace it' })).not.toBeInTheDocument();
    expect(calls(invoke, 'memory_script_save').filter((args) => (args as { replace?: boolean }).replace === true)).toHaveLength(0);

    // Back on the project it was found for, the finding stands again.
    await pickTarget(dialog, /^demo/);
    expect(within(dialog).getByText('This name is taken')).toBeInTheDocument();
  });
});
