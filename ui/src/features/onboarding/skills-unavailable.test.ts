import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import type { HarnessSetup } from '$lib/ipc/bindings/HarnessSetup';
import type { SkillsSetup } from '$lib/ipc/bindings/SkillsSetup';

import SkillsCard from './SkillsCard.svelte';
import { skillsUnavailable } from './setup-model';

function opencodeWith(skills: SkillsSetup): HarnessSetup {
  const missing = { type: 'missing', file: '~/.config/opencode/opencode.json', exists: true } as const;

  return { harness: 'opencode', installed: true, skills, mcp: missing, memoryMcp: missing, desktopMcp: missing };
}

/** Renders the card with its details open, which is where the reason is. */
async function openCard(skills: SkillsSetup): Promise<void> {
  render(SkillsCard, { harnesses: [opencodeWith(skills)] });
  await fireEvent.click(screen.getByRole('button', { name: /Szczegóły|Details/ }));
}

const NO_NODE: SkillsSetup = { type: 'unavailable', reason: 'Needs Node.js 22 or newer.', code: 'nodeMissing' };

afterEach(async () => {
  await i18n.load('en');
});

describe('Why the skills cannot be installed', () => {
  it('says a missing Node.js in the app’s language, on the skills card and in the installer list', async () => {
    await i18n.load('pl');
    await openCard(NO_NODE);

    expect(await screen.findByText('Wymaga Node.js 22 lub nowszego.')).toBeInTheDocument();
    expect(screen.queryByText('Needs Node.js 22 or newer.')).not.toBeInTheDocument();
    expect(skillsUnavailable([opencodeWith(NO_NODE)])).toEqual([{ harness: 'opencode', reason: 'Wymaga Node.js 22 lub nowszego.' }]);
  });

  it('says it in English by default', async () => {
    await openCard(NO_NODE);

    expect(await screen.findByText('Needs Node.js 22 or newer.')).toBeInTheDocument();
  });

  it('shows the backend’s English reason when there is no code or the code is unknown', async () => {
    await i18n.load('pl');

    const unknown = { type: 'unavailable', reason: 'Something new went wrong.', code: 'somethingNew' } as unknown as SkillsSetup;
    const noCode: SkillsSetup = { type: 'unavailable', reason: 'Not supported yet.', code: null };

    await openCard(unknown);
    expect(await screen.findByText('Something new went wrong.')).toBeInTheDocument();

    expect(skillsUnavailable([opencodeWith(noCode)])).toEqual([{ harness: 'opencode', reason: 'Not supported yet.' }]);
  });
});
