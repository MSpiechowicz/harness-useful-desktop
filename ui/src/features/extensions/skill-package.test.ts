import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import type { PackageInfo } from '$lib/ipc/bindings/PackageInfo';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { extensionsView } from '$lib/stores/views.svelte';

import { freshBackend } from '../../../tests/ecosystem-backend';
import { samplePrefs } from '../../../tests/helpers';
import { actionRunner } from './actions/action-runner.svelte';
import { groupFolds } from './group-folds.svelte';
import ExtensionsViewer from './ExtensionsViewer.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  await harness.load();
});

afterEach(() => {
  actionRunner.close();
  project.active = null;
  vi.restoreAllMocks();
});

const kit: PackageInfo = { name: 'pi-local-kit', source: { kind: 'text', value: 'git:github.com/demo/pi-local-kit@v1' }, kind: 'git', scope: 'user', installed: true, resources: ['skills'] };
const other: PackageInfo = { name: 'other-package', source: { kind: 'text', value: 'npm:@demo/other-package@1.0.0' }, kind: 'npm', scope: 'user', installed: true, resources: [] };

/** PI's inventory in which `kit-skill` comes from the package `pi-local-kit` and `solo-skill` from none. */
async function openPi(): Promise<void> {
  freshBackend();
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'pi' });
  groupFolds.reset();
  render(ExtensionsViewer);
  await screen.findByRole('navigation', { name: 'Extensions' });

  const view = extensionsView.data!;
  const [template] = view.skills;
  extensionsView.data = {
    ...view,
    packages: [other, kit],
    skills: [
      { ...template, name: 'kit-skill', description: 'Comes in a package.', path: '/pkg/kit-skill/SKILL.md', package: 'pi-local-kit' },
      { ...template, name: 'solo-skill', description: 'Stands alone.', path: '/skills/solo-skill/SKILL.md', package: null },
    ],
  } as ExtensionsView;
}

async function chooseSkill(name: string): Promise<void> {
  const group = await screen.findByRole('region', { name: /Skills/ });
  await fireEvent.click(within(group).getByRole('button', { name: new RegExp(name) }));
}

describe('Install button', () => {
  it('reads Install with one add option, and opens it from the menu like a harness with several', async () => {
    freshBackend();
    prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'pi' });
    render(ExtensionsViewer);
    await screen.findByRole('navigation', { name: 'Extensions' });

    expect(screen.queryByRole('button', { name: 'Install package' })).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Install' }));
    const entries = await screen.findAllByRole('menuitem');
    expect(entries.map((entry) => entry.textContent?.trim())).toEqual(['Install package']);

    await fireEvent.click(entries[0]);
    expect(await screen.findByRole('dialog')).toBeInTheDocument();
  });
});

describe('a skill that comes from a package', () => {
  it('says which package, notes that removing it removes the skills, and offers its actions', async () => {
    await openPi();
    await chooseSkill('kit-skill');

    expect(await screen.findByRole('button', { name: 'From package pi-local-kit' })).toBeInTheDocument();
    expect(screen.getByText('Removing the package removes every skill it provides.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Update' })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Uninstall' }));
    expect(await within(await screen.findByRole('dialog')).findByText('pi remove git:github.com/demo/pi-local-kit@v1')).toBeInTheDocument();
  });

  it('selects the package in the list', async () => {
    await openPi();
    await chooseSkill('kit-skill');

    await fireEvent.click(await screen.findByRole('button', { name: 'From package pi-local-kit' }));

    await waitFor(() => expect(screen.getByRole('heading', { level: 2, name: 'pi-local-kit' })).toBeInTheDocument());
    expect(screen.queryByRole('button', { name: /^From package/ })).toBeNull();
  });

  it('shows neither the line nor the package actions for a skill without a package', async () => {
    await openPi();
    await chooseSkill('solo-skill');

    await waitFor(() => expect(screen.getByRole('heading', { level: 2, name: 'solo-skill' })).toBeInTheDocument());

    expect(screen.queryByRole('button', { name: /^From package/ })).toBeNull();
    expect(screen.queryByText('Removing the package removes every skill it provides.')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Uninstall' })).toBeNull();
  });
});
