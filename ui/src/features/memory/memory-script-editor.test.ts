import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

import { sampleScripts } from '$lib/ipc/mock/memory-scripts-fixtures';
import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';
import type { ScriptSaved } from '$lib/ipc/bindings/ScriptSaved';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { IpcFailure } from '$lib/ipc/transport';
import { scriptList } from '$lib/stores/memoryScripts.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { project } from '$lib/stores/project.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { scriptEditor } from './editor-state.svelte';
import { calls, fakeMemory, PROJECT, registerMemoryHooks, rowButton, showMemory } from './memory-test-support';
import ScriptsView from './ScriptsView.svelte';

registerMemoryHooks();

const BODY = '#!/usr/bin/env bash\necho todo';
const OPEN_PRS_SHA = sampleScripts().find((script) => script.name === 'open-prs')?.sha256;

/** Opens the editor on a new script from the primary action of the header. */
async function openEditor(): Promise<HTMLElement> {
  await showMemory();
  await fireEvent.click(await screen.findByRole('button', { name: 'Add script' }));

  return screen.findByRole('dialog', { name: 'Add a script' });
}

async function fill(dialog: HTMLElement, fields: { name?: string; description?: string; body?: string; timeout?: string }): Promise<void> {
  const labels = { name: 'Name', description: 'Description', body: 'Code', timeout: 'Time limit' } as const;

  for (const [field, value] of Object.entries(fields)) {
    await fireEvent.input(within(dialog).getByLabelText(labels[field as keyof typeof labels]), { target: { value } });
  }
}

/** Opens the Scripts view where there is no table to show, only words, and waits for them. */
async function showWithoutTable(words: string): Promise<{ unmount: () => void }> {
  nav.go('scripts');
  const view = render(ScriptsView);
  await screen.findByText(words);

  return view;
}

/** Makes the next saves answer with `answer`, whatever was asked. */
function answerSaves(invoke: ReturnType<typeof fakeMemory>, answer: () => ScriptSaved): void {
  const base = invoke.getMockImplementation() as (name: CommandName, args: unknown) => Promise<unknown>;

  invoke.mockImplementation((async (name: CommandName, args: unknown) => (name === 'memory_script_save' ? answer() : base(name, args))) as typeof ipc.invoke);
}

const toastTitles = () => toasts.items.map((toast) => toast.title);

/** Opens the editor on a script of the list, from the Edit button of its panel. */
async function openEdit(name: string): Promise<HTMLElement> {
  await showMemory();
  await fireEvent.click(rowButton(name) as HTMLElement);
  const panel = await screen.findByRole('region', { name });
  await waitFor(() => expect(within(panel).getByRole('button', { name: 'Edit' })).toBeEnabled());
  await fireEvent.click(within(panel).getByRole('button', { name: 'Edit' }));

  return screen.findByRole('dialog', { name: 'Edit script' });
}

describe('adding a script', () => {
  it('asks for the name first and does not add anything until the script is complete', async () => {
    fakeMemory();
    const dialog = await openEditor();

    expect(within(dialog).getByLabelText('Name')).toHaveFocus();
    expect(within(dialog).getByRole('button', { name: 'Add script' })).toBeDisabled();

    await fill(dialog, { name: 'list-todos', description: 'Lists the todos.' });
    expect(within(dialog).getByRole('button', { name: 'Add script' })).toBeDisabled();

    await fill(dialog, { body: BODY });
    expect(within(dialog).getByRole('button', { name: 'Add script' })).toBeEnabled();
    expect(within(dialog).getByText('The code you write here is trusted on this computer, so agents can run it right away.')).toBeInTheDocument();
  });

  it('saves the script in the open repository, says so, and opens its panel with the code', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { name: 'list-todos', description: ' Lists the todos. ', body: BODY, timeout: '120' });
    await fireEvent.click(within(dialog).getByRole('radio', { name: 'Python' }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add script' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Add a script' })).not.toBeInTheDocument());
    expect(calls(invoke, 'memory_script_save')).toEqual([
      { project: PROJECT, replace: false, draft: { name: 'list-todos', description: 'Lists the todos.', language: 'python', body: BODY, timeoutSecs: 120, scope: 'repo' } },
    ]);
    expect(calls(invoke, 'memory_script_save')[0]).not.toHaveProperty('base');
    expect(toasts.items.map((toast) => [toast.title, toast.description, toast.tone])).toContainEqual(['Script added', 'list-todos', 'success']);

    const panel = await screen.findByRole('region', { name: 'list-todos' });
    expect(within(panel).getByText('Python · github.com/useful/harness-desktop')).toBeInTheDocument();
    expect(await within(panel).findByText(/echo todo/)).toBeInTheDocument();
  });

  it('uses a minute when the time limit is left empty', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { name: 'list-todos', description: 'Lists the todos.', body: BODY });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add script' }));

    await waitFor(() => expect(calls(invoke, 'memory_script_save')).toHaveLength(1));
    expect(calls(invoke, 'memory_script_save')[0]).toMatchObject({ draft: { timeoutSecs: 60, language: 'bash' } });
  });

  it('says what is wrong with a field once the user has been at it, and keeps the script from being added', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    expect(within(dialog).queryByText('Give the script a name.')).not.toBeInTheDocument();
    await fireEvent.blur(within(dialog).getByLabelText('Name'));
    expect(within(dialog).getByText('Give the script a name.')).toBeInTheDocument();

    await fill(dialog, { name: 'Bad Name', description: 'x'.repeat(501), body: 'é'.repeat(33_000), timeout: '601' });

    expect(within(dialog).getByText('Use lowercase letters, digits, - and _, starting with a letter or digit.')).toBeInTheDocument();
    expect(within(dialog).getByText('The description is longer than 500 characters.')).toBeInTheDocument();
    expect(within(dialog).getByText('The code is larger than 64 KiB.')).toBeInTheDocument();
    expect(within(dialog).getByText('Use a whole number of seconds from 1 to 600.')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Add script' })).toBeDisabled();

    await fireEvent.submit(within(dialog).getByLabelText('Name').closest('form') as HTMLFormElement);
    expect(calls(invoke, 'memory_script_save')).toHaveLength(0);
  });

  it('flags hidden characters in the code and shows them written out, without keeping the script from being saved', async () => {
    fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { name: 'list-todos', description: 'Lists the todos.', body: `echo ok​‮ done` });

    expect(within(dialog).getByRole('alert')).toHaveTextContent('The code has 2 hidden characters.');
    expect(within(dialog).getByLabelText('The code with hidden characters written out')).toHaveTextContent('echo ok⟨U+200B⟩⟨U+202E⟩ done');
    expect(within(dialog).getByRole('button', { name: 'Add script' })).toBeEnabled();
  });

  it('offers to open or to replace a script that has the name, and writes nothing until one is chosen', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { name: 'ci-status', description: 'A newer one.', body: BODY });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add script' }));

    expect(await within(dialog).findByText('This name is taken')).toBeInTheDocument();
    expect(toastTitles()).not.toContain('Script added');

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Replace it' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Add a script' })).not.toBeInTheDocument());
    expect(calls(invoke, 'memory_script_save').at(-1)).toMatchObject({ replace: true, draft: { name: 'ci-status', description: 'A newer one.' } });
    expect(calls(invoke, 'memory_script_save').at(-1)).not.toHaveProperty('base');
    expect(toastTitles()).toContain('Script saved');
    expect(viewState.scriptId).toBe(15);
    expect(await screen.findByText('A newer one.')).toBeInTheDocument();
  });

  it('opens the script that has the name instead of saving', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { name: 'ci-status', description: 'A newer one.', body: BODY });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add script' }));
    await fireEvent.click(await within(dialog).findByRole('button', { name: 'Open it' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Add a script' })).not.toBeInTheDocument());
    expect(await screen.findByRole('region', { name: 'ci-status' })).toBeInTheDocument();
    expect(calls(invoke, 'memory_script_save')).toHaveLength(1);
  });

  it('keeps the dialog open and says why when the script could not be saved', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();
    const base = invoke.getMockImplementation() as (name: CommandName, args: unknown) => Promise<unknown>;
    invoke.mockImplementation((async (name: CommandName, args: unknown) => {
      if (name === 'memory_script_save') {
        throw IpcFailure.from({ code: 'process', message: 'harness-memory stopped' });
      }

      return base(name, args);
    }) as typeof ipc.invoke);

    await fill(dialog, { name: 'list-todos', description: 'Lists the todos.', body: BODY });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add script' }));

    await waitFor(() => expect(toasts.items.map((toast) => [toast.title, toast.description])).toContainEqual(['The script was not saved', 'harness-memory stopped']));
    expect(screen.getByRole('dialog', { name: 'Add a script' })).toBeInTheDocument();
  });

  it('says in the toast that the script could not be trusted when the server still wants it approved', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();
    const saved = invoke.getMockImplementation() as (name: CommandName, args: unknown) => Promise<ScriptSaved>;
    const written = (await saved('memory_script_save', { project: PROJECT, replace: false, draft: { name: 'other-one', description: 'd', language: 'bash', body: BODY, timeoutSecs: 60, scope: 'repo' } })) as ScriptSaved;

    answerSaves(invoke, () => ({ ...written, script: written.script && { ...written.script, trusted: false, approval: 'needed' } }));
    await fill(dialog, { name: 'list-todos', description: 'Lists the todos.', body: BODY });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add script' }));

    await waitFor(() => expect(toastTitles()).toContain('Script saved but not trusted'));
    expect(toastTitles()).not.toContain('Script added');
    expect(toasts.items.find((item) => item.title === 'Script saved but not trusted')).toMatchObject({ tone: 'warning', description: expect.stringContaining('could not be trusted on this computer') });
  });
});

describe('editing a script', () => {
  it('opens on the script as it is, with its name and scope kept', async () => {
    fakeMemory();
    const dialog = await openEdit('open-prs');

    expect(within(dialog).getByLabelText('Name')).toHaveValue('open-prs');
    expect(within(dialog).getByLabelText('Name')).toHaveAttribute('readonly');
    expect(within(dialog).getByLabelText('Description')).toHaveValue('Lists the open pull requests of this repository with their review state.');
    expect(within(dialog).getByLabelText('Description')).toHaveFocus();
    expect((within(dialog).getByLabelText('Code') as HTMLTextAreaElement).value).toContain('gh pr list');
    expect(within(dialog).getByLabelText('Time limit')).toHaveValue('60');
    expect(within(dialog).getByRole('radio', { name: 'Bash' })).toBeChecked();
    expect(within(dialog).getByRole('radio', { name: 'Repository' })).toBeDisabled();
    expect(within(dialog).getByRole('radio', { name: 'Global' })).toBeDisabled();
  });

  it('saves the changes over the script of that name, says so, and shows them in its panel', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit('open-prs');

    await fill(dialog, { description: 'Open pull requests, newest first.', timeout: '90' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Edit script' })).not.toBeInTheDocument());
    expect(calls(invoke, 'memory_script_save')).toEqual([
      { project: PROJECT, replace: true, base: OPEN_PRS_SHA, draft: expect.objectContaining({ name: 'open-prs', scope: 'repo', language: 'bash', description: 'Open pull requests, newest first.', timeoutSecs: 90 }) },
    ]);
    expect(toastTitles()).toContain('Script saved');
    expect(await screen.findByText('Open pull requests, newest first.')).toBeInTheDocument();
    expect(scriptEditor.open).toBe(false);
  });
});

describe('a script that changed while it was being edited', () => {
  const NEWER = 'f'.repeat(64);

  /** Makes the next save of an edit refuse as the server does when the script is another one by now, and the script read again answer `now` (or be gone). */
  function changeUnderneath(invoke: ReturnType<typeof fakeMemory>, now: 'gone' | Partial<ScriptDetail>): void {
    const base = invoke.getMockImplementation() as (name: CommandName, args: unknown) => Promise<unknown>;
    let refused = false;

    invoke.mockImplementation((async (name: CommandName, args: unknown) => {
      if (name === 'memory_script_save' && !refused) {
        refused = true;

        throw IpcFailure.from({ code: 'conflict', message: 'the script changed since you opened it' });
      }

      if (name === 'memory_script_get') {
        if (now === 'gone') {
          throw IpcFailure.from({ code: 'notFound', message: 'gone' });
        }

        return { ...((await base(name, args)) as ScriptDetail), sha256: NEWER, ...now };
      }

      return base(name, args);
    }) as typeof ipc.invoke);
  }

  it('keeps the dialog and the edits, says so inline and not as a failure, and writes nothing', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit('open-prs');

    changeUnderneath(invoke, { description: 'Changed by an agent.' });
    await fill(dialog, { description: 'My edit.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    expect(await within(dialog).findByText('This script changed since you opened it. Reload it to see the latest version.')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('Description')).toHaveValue('My edit.');
    expect(screen.getByRole('dialog', { name: 'Edit script' })).toBeInTheDocument();
    expect(toastTitles()).not.toContain('Script saved');
    expect(toastTitles()).not.toContain('The script was not saved');
  });

  it('fills the form with the latest version on Reload, and saves the next edit against that version', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit('open-prs');

    changeUnderneath(invoke, { description: 'Changed by an agent.' });
    await fill(dialog, { description: 'My edit.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
    await fireEvent.click(await within(dialog).findByRole('button', { name: 'Reload' }));

    await waitFor(() => expect(within(dialog).getByLabelText('Description')).toHaveValue('Changed by an agent.'));
    expect(within(dialog).queryByRole('button', { name: 'Reload' })).not.toBeInTheDocument();

    await fill(dialog, { timeout: '90' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Edit script' })).not.toBeInTheDocument());
    expect(calls(invoke, 'memory_script_save').map((call) => (call as { base?: string }).base)).toEqual([OPEN_PRS_SHA, NEWER]);
  });

  it('says the script no longer exists when it was deleted, cannot be saved, and offers Close', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit('open-prs');

    changeUnderneath(invoke, 'gone');
    await fill(dialog, { description: 'My edit.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
    await fireEvent.click(await within(dialog).findByRole('button', { name: 'Reload' }));

    expect(await within(dialog).findByText('This script no longer exists. It was deleted since you opened it.')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Save' })).toBeDisabled();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Edit script' })).not.toBeInTheDocument());
  });
});

describe('where a script can be added', () => {
  it('is not offered while Harness Memory is missing, outdated or not answering', async () => {
    const cases = [
      [{ kind: 'notInstalled' }, 'Harness Memory is not installed'],
      [{ kind: 'outdated' }, 'Your memory needs an update'],
      [{ kind: 'failed', message: 'it crashed' }, 'Could not reach Harness Memory'],
    ] as const;

    for (const [availability, words] of cases) {
      fakeMemory(availability);
      const { unmount } = await showWithoutTable(words);

      expect(screen.queryByRole('button', { name: 'Add script' })).not.toBeInTheDocument();

      unmount();
    }
  });

  it('is offered in the empty list too', async () => {
    fakeMemory({ kind: 'available' }, []);
    await showWithoutTable('No scripts yet');

    const buttons = await screen.findAllByRole('button', { name: 'Add script' });
    expect(buttons).toHaveLength(2);

    await fireEvent.click(buttons[1]);

    expect(await screen.findByRole('dialog', { name: 'Add a script' })).toBeInTheDocument();
  });

  it('is opened by the command palette once the list is up', async () => {
    fakeMemory();
    scriptEditor.wantsNew = true;
    await showMemory();

    expect(await screen.findByRole('dialog', { name: 'Add a script' })).toBeInTheDocument();
    expect(scriptEditor.wantsNew).toBe(false);
  });
});

describe('a script that waits for review', () => {
  const WAITING = 'This script was written by an agent and waits for your review. Saving changes to its code makes it yours and trusted. Saving without changing the code keeps it waiting.';

  /** Makes the next saves answer as the server does for this script: written, but still without the user's trust. */
  function answerNeeded(invoke: ReturnType<typeof fakeMemory>): void {
    const written = sampleScripts().find((script) => script.name === 'deploy-check');

    answerSaves(invoke, () => ({ outcome: 'replaced', id: 27, script: written ? { ...written, trusted: false, approval: 'needed' } : null }));
  }

  it('says in the editor that saving the code makes it the user\'s own and saving without changes keeps it waiting', async () => {
    fakeMemory();
    const dialog = await openEdit('deploy-check');

    expect(within(dialog).getByText('Not approved on this computer')).toBeInTheDocument();
    expect(within(dialog).getByText(WAITING)).toBeInTheDocument();
  });

  it('does not say it for a script the user already trusts', async () => {
    fakeMemory();
    const dialog = await openEdit('open-prs');

    expect(within(dialog).queryByText(WAITING)).not.toBeInTheDocument();
  });

  it('tells in an info toast that it still waits when it was saved without a change to the code', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit('deploy-check');

    answerNeeded(invoke);
    await fill(dialog, { description: 'Checks the staging deploy.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(toastTitles()).toContain('Saved, still waits for your review'));
    expect(toasts.items.find((item) => item.title === 'Saved, still waits for your review')).toMatchObject({ tone: 'info' });
    expect(toastTitles()).not.toContain('Script saved but not trusted');
  });

  it('counts a change of line endings and trailing whitespace as no change to the code', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit('deploy-check');
    const code = within(dialog).getByLabelText('Code') as HTMLTextAreaElement;

    answerNeeded(invoke);
    await fill(dialog, { body: `${code.value.replace(/\n/g, '  \r\n')}\n\n` });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(toastTitles()).toContain('Saved, still waits for your review'));
    expect(toastTitles()).not.toContain('Script saved but not trusted');
  });

  it('keeps the failure wording when the code was changed and it still came back waiting', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit('deploy-check');

    answerNeeded(invoke);
    await fill(dialog, { body: `${BODY}\necho changed` });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(toastTitles()).toContain('Script saved but not trusted'));
    expect(toasts.items.find((item) => item.title === 'Script saved but not trusted')).toMatchObject({ tone: 'warning' });
    expect(toastTitles()).not.toContain('Saved, still waits for your review');
  });
});

describe('a script in a language this version does not know', () => {
  it('is kept as it is: the editor says so and cannot save it', async () => {
    const scripts = sampleScripts().map((script) => (script.name === 'open-prs' ? { ...script, language: 'ruby' } : script));
    const invoke = fakeMemory(undefined, scripts);
    const dialog = await openEdit('open-prs');

    expect(within(dialog).getByText('Language not supported here')).toBeInTheDocument();
    expect(within(dialog).getByText(/“ruby”, which cannot be edited here/)).toBeInTheDocument();
    expect(within(dialog).queryByRole('radio', { name: 'Bash' })).toBeNull();
    expect(within(dialog).getByRole('button', { name: 'Save' })).toBeDisabled();

    await fireEvent.submit(within(dialog).getByLabelText('Name').closest('form') as HTMLFormElement);

    expect(calls(invoke, 'memory_script_save')).toHaveLength(0);
  });
});

describe('an editor that outlives nothing', () => {
  afterEach(async () => {
    await project.open(PROJECT);
  });

  it('is not opened later when Harness Memory comes back after it was not answering', async () => {
    let broken = true;
    fakeMemory(() => {
      if (broken) {
        throw IpcFailure.from({ code: 'process', message: 'harness-memory stopped' });
      }

      return { kind: 'available' };
    });
    scriptEditor.wantsNew = true;
    nav.go('scripts');
    render(ScriptsView);
    await screen.findByText('Could not read the scripts');

    expect(scriptEditor.wantsNew).toBe(false);

    broken = false;
    await scriptList.refresh();
    await screen.findByRole('grid', { name: 'Scripts' });

    expect(screen.queryByRole('dialog', { name: 'Add a script' })).not.toBeInTheDocument();
  });

  it('closes when the view is left', async () => {
    fakeMemory();
    await openEditor();

    cleanup();

    expect(scriptEditor.open).toBe(false);
    expect(scriptEditor.target).toBeNull();
  });

  it('closes when the project changes', async () => {
    fakeMemory();
    await openEditor();

    project.active = null;

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Add a script' })).not.toBeInTheDocument());
    expect(scriptEditor.open).toBe(false);
  });
});
