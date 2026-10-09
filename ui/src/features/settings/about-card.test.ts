import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { openExternal } = vi.hoisted(() => ({ openExternal: vi.fn() }));
vi.mock('$lib/ipc/actions', () => ({ openExternal }));

import AboutCard from './AboutCard.svelte';

beforeEach(() => openExternal.mockReset());

describe('AboutCard links', () => {
  it('opens the repository, the releases and the licence, all on the project allowlist', async () => {
    render(AboutCard);
    const links = within(screen.getByRole('list', { name: 'Project links' }));

    for (const name of [/source code/i, /releases/i, /licence/i]) {
      await fireEvent.click(links.getByRole('button', { name }));
    }

    expect(openExternal).toHaveBeenCalledTimes(3);
    for (const [url] of openExternal.mock.calls) {
      expect(url).toMatch(/^https:\/\/github\.com\/MSpiechowicz\/harness-useful-desktop(\/|$)/);
    }
  });
});
