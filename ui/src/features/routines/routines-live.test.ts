import { fireEvent, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { viewState } from '$lib/stores/view-state.svelte';

import { routines } from './routines-data.svelte';
import { fakeRoutines, registerRoutinesHooks, rowOf, showRoutines } from './routines-test-support';

registerRoutinesHooks();

describe('an open routine', () => {
  it('shows what was changed in its file while its panel was open, at the next poll', async () => {
    const { state } = fakeRoutines();
    await showRoutines();
    const stop = routines.start();
    const target = routines.views.find((view) => view.status === 'active' && view.routine?.action.type === 'command') ?? routines.views[0];
    const name = target.routine?.name ?? '';
    await fireEvent.click(rowOf(name));
    const panel = await screen.findByRole('region', { name });

    state.rewrite(target.id, { name: 'Renamed elsewhere', description: 'Edited in an editor.' });
    window.dispatchEvent(new Event('focus'));

    expect(await screen.findByRole('region', { name: 'Renamed elsewhere' })).toBe(panel);
    expect(await within(panel).findByText('Edited in an editor.')).toBeInTheDocument();
    expect(viewState.routineId).toBe(target.id);
    stop();
  });
});
