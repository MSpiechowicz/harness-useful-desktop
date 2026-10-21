import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { MEMORY_SETTINGS_PART } from '$lib/chat/hook-context';

import HookCard from './HookCard.svelte';

const MEMORY = { name: MEMORY_SETTINGS_PART, text: 'Harness Desktop: saving notes is off for this chat. Scripts are saved.' };
const HOOK = { name: 'Search Harness Memory first', text: 'Call note_search before anything else.' };

async function opened(hooks: (typeof MEMORY)[]): Promise<void> {
  render(HookCard, { hooks, kind: 'context' });
  await fireEvent.click(screen.getAllByRole('button')[0]);
}

describe('the chip of what was put in front of a message', () => {
  it('says the app added the Memory settings when nothing else was added', async () => {
    await opened([MEMORY]);

    expect(screen.getByText('Added by Harness Desktop')).toBeInTheDocument();
    expect(screen.getByText('Your Memory settings, told to the agent with this message.')).toBeInTheDocument();
    expect(screen.getByText('Memory')).toBeInTheDocument();
    expect(screen.queryByText(/hooks?$/)).not.toBeInTheDocument();
  });

  it('names hooks and the Memory settings apart, and counts only the hooks', async () => {
    await opened([MEMORY, HOOK]);

    expect(screen.getByText('Added by hooks')).toBeInTheDocument();
    expect(screen.getByText('Text from hooks and your Memory settings, put in front of this message for the agent.')).toBeInTheDocument();
    expect(screen.getByText('1 hook')).toBeInTheDocument();
  });

  it('keeps the hooks wording when there are no Memory settings', async () => {
    await opened([HOOK]);

    expect(screen.getByText('Text from hooks, put in front of this message for the agent.')).toBeInTheDocument();
    expect(screen.getByText('1 hook')).toBeInTheDocument();
  });
});
