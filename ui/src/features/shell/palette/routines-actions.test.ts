import { describe, expect, it } from 'vitest';

import { routines } from '../../routines/routines-data.svelte';
import { nav } from '$lib/stores/nav.svelte';

import { actionCommands, navigationCommands } from './actions';

describe('the command palette and Routines', () => {
  it('offers to go to Routines, with its shortcut, and to start a new routine', () => {
    const go = navigationCommands().find((command) => command.id === 'go-routines');
    const create = actionCommands({ openShortcuts() {} }).find((command) => command.id === 'new-routine');

    expect(go).toMatchObject({ label: 'Go to Routines' });
    expect(go?.shortcut).toBeUndefined();
    expect(create).toMatchObject({ label: 'New routine' });

    create?.run();

    expect(nav.current).toBe('routines');
    expect(routines.wantsNew).toBe(true);
    routines.wantsNew = false;
    nav.go('chat');
  });

  it('keeps Ctrl+comma for Settings in the list of views', () => {
    expect(navigationCommands().find((command) => command.id === 'go-settings')?.shortcut).toBe('mod+,');
  });
});
