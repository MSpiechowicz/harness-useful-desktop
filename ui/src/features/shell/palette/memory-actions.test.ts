import { describe, expect, it } from 'vitest';

import { nav } from '$lib/stores/nav.svelte';

import { noteEditor, scriptEditor } from '../../memory/editor-state.svelte';
import { actionCommands } from './actions';

describe('the command palette and the Notes and Scripts views', () => {
  it('offers to start a new note, and takes the user to Notes to write it', () => {
    const create = actionCommands({ openShortcuts() {} }).find((command) => command.id === 'new-note');

    expect(create).toMatchObject({ label: 'New note' });

    create?.run();

    expect(nav.current).toBe('notes');
    expect(noteEditor.wantsNew).toBe(true);
    noteEditor.reset();
    nav.go('chat');
  });

  it('offers to start a new script, and takes the user to Scripts to write it', () => {
    const create = actionCommands({ openShortcuts() {} }).find((command) => command.id === 'new-script');

    expect(create).toMatchObject({ label: 'New script' });

    create?.run();

    expect(nav.current).toBe('scripts');
    expect(scriptEditor.wantsNew).toBe(true);
    scriptEditor.reset();
    nav.go('chat');
  });
});
