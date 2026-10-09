import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { freshBackend } from '../../../../tests/ecosystem-backend';
import { samplePrefs } from '../../../../tests/helpers';
import ExtensionsViewer from '../ExtensionsViewer.svelte';
import { actionRunner } from './action-runner.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  await harness.load();
});

afterEach(() => {
  actionRunner.close();
  project.active = null;
  vi.restoreAllMocks();
});

async function open(id: 'omp' | 'pi', tab: RegExp, item: RegExp) {
  const backend = freshBackend();
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: id });
  render(ExtensionsViewer);

  await fireEvent.click(await screen.findByRole('tab', { name: tab }));
  await fireEvent.click(await screen.findByRole('button', { name: item }));

  return backend;
}

const dialog = () => screen.findByRole('dialog');

/** Confirms once the preview has finished and the button is live. */
async function confirmWith(name: string): Promise<void> {
  const button = await within(await dialog()).findByRole('button', { name });
  await waitFor(() => expect(button).toBeEnabled());
  await fireEvent.click(button);
}

describe('extension actions', () => {
  it('previews first, runs only after the confirmation, then shows the refreshed list', async () => {
    const { calls } = await open('omp', /Plugins/, /npm-helper/);
    expect(screen.getByRole('heading', { level: 2, name: 'npm-helper' }).parentElement).toHaveTextContent('Disabled');

    await fireEvent.click(screen.getByRole('button', { name: 'Enable' }));

    const confirm = await dialog();
    expect(await within(confirm).findByText('omp plugin enable npm-helper')).toBeInTheDocument();
    expect(within(confirm).getByText(/User scope/)).toBeInTheDocument();
    expect(within(confirm).getByText('No preview available — the command will run as shown.')).toBeInTheDocument();
    expect(calls('extension_action')).toEqual([expect.objectContaining({ dryRun: true, harness: 'omp', project: null })]);

    await confirmWith('Enable');

    await within(await dialog()).findByText('Confirmed: the refreshed list shows the change.');
    expect(calls('extension_action').map((call) => call.dryRun)).toEqual([true, false]);

    await fireEvent.click(screen.getByRole('button', { name: 'Close' }));

    await waitFor(() => expect(screen.getByRole('heading', { level: 2, name: 'npm-helper' }).parentElement).toHaveTextContent('Enabled'));
    expect(screen.getByRole('button', { name: 'Disable' })).toBeInTheDocument();
  });

  it('runs nothing when the confirmation is cancelled', async () => {
    const { calls } = await open('omp', /Plugins/, /npm-helper/);

    await fireEvent.click(screen.getByRole('button', { name: 'Enable' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Cancel' }));

    expect(screen.queryByRole('dialog')).toBeNull();
    expect(calls('extension_action').every((call) => call.dryRun)).toBe(true);
  });

  it('shows the CLI preview where there is one and confirms a removal in the danger tone', async () => {
    await open('omp', /Plugins/, /npm-helper/);

    await fireEvent.click(screen.getByRole('button', { name: 'Uninstall' }));

    const confirm = await dialog();
    expect(await within(confirm).findByText('[dry-run] Would uninstall npm-helper')).toBeInTheDocument();
    expect(within(confirm).getByRole('button', { name: 'Uninstall' }).className).toContain('bg-danger-fill');
  });

  it('confirms the command that will really run, never the preview flag, and then reports the one that ran', async () => {
    await open('omp', /Plugins/, /npm-helper/);

    await fireEvent.click(screen.getByRole('button', { name: 'Uninstall' }));

    const confirm = await dialog();
    await within(confirm).findByText('[dry-run] Would uninstall npm-helper');
    const command = within(confirm).getByRole('region', { name: 'Command' });
    expect(command).toHaveTextContent('Command that will run');
    expect(command).toHaveTextContent('omp plugin uninstall npm-helper');
    expect(command).not.toHaveTextContent('--dry-run');
    // The flag is still disclosed, as what produced the preview above.
    expect(within(within(confirm).getByRole('region', { name: 'Preview' })).getByText('omp plugin uninstall npm-helper --dry-run')).toBeInTheDocument();

    await confirmWith('Uninstall');

    const done = within(await dialog());
    await done.findByText('Confirmed: the refreshed list shows the change.');
    expect(done.getByRole('region', { name: 'Command' })).toHaveTextContent('Command that ran');
    expect(done.getByRole('region', { name: 'Command' })).toHaveTextContent('omp plugin uninstall npm-helper');
    expect(done.getByRole('region', { name: 'Command' })).not.toHaveTextContent('--dry-run');
  });

  it('shows an OMP failure reported with exit code 0 and keeps the log open', async () => {
    await open('omp', /Plugins/, /escaping-plugin/);

    await fireEvent.click(screen.getByRole('button', { name: 'Disable' }));
    await confirmWith('Disable');

    const result = await dialog();
    expect(await within(result).findByText('The action failed')).toBeInTheDocument();
    expect(within(result).getAllByText(/✘ Failed to disable escaping-plugin@demo/).length).toBeGreaterThan(0);
    expect(within(result).getByRole('button', { name: 'Output log' })).toHaveAttribute('aria-expanded', 'true');
  });

  it('installs from a marketplace entry in the chosen scope, and offers project scope only with a project', async () => {
    const { calls } = await open('omp', /Marketplaces/, /^demo/);
    await fireEvent.click(await screen.findByRole('button', { name: 'Install commit-helper' }));

    expect(screen.getByRole('menuitem', { name: /In this project/ })).toBeDisabled();

    await fireEvent.click(screen.getByRole('menuitem', { name: /For me/ }));

    const confirm = await dialog();
    expect(await within(confirm).findByText('omp plugin install commit-helper@demo --scope user')).toBeInTheDocument();
    expect(within(confirm).getByText('[dry-run] Would install commit-helper@demo')).toBeInTheDocument();
    expect(calls('extension_action')[0].action).toEqual({ type: 'pluginInstall', target: 'commit-helper@demo', scope: 'user' });
  });

  it('runs a project-scope install inside the open project', async () => {
    const { calls } = await open('omp', /Marketplaces/, /^demo/);
    project.active = fixtures.recentProjects[0];

    await fireEvent.click(await screen.findByRole('button', { name: 'Install commit-helper' }));
    await fireEvent.click(screen.getByRole('menuitem', { name: /In this project/ }));

    const confirm = await dialog();
    expect(await within(confirm).findByText(/Project scope: runs inside/)).toBeInTheDocument();
    expect(calls('extension_action')[0]).toMatchObject({ project: fixtures.recentProjects[0].path, action: { scope: 'project' } });
  });

  it('adds a marketplace from the toolbar, refusing unsafe text before anything is sent', async () => {
    const { calls } = await open('omp', /Marketplaces/, /^demo/);

    await fireEvent.click(screen.getByRole('button', { name: 'Add marketplace' }));
    const input = await screen.findByRole('textbox', { name: /Marketplace source/ });

    await fireEvent.input(input, { target: { value: '--evil' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Review' }));

    expect(await screen.findByText('It must not start with a dash.')).toBeInTheDocument();
    expect(calls('extension_action')).toHaveLength(0);

    await fireEvent.input(input, { target: { value: 'acme/plugins' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Review' }));

    expect(await screen.findByText('omp plugin marketplace add acme/plugins')).toBeInTheDocument();
    expect(calls('extension_action')[0]).toMatchObject({ dryRun: true, action: { type: 'marketplaceAdd', source: 'acme/plugins' } });
  });

  it('removes a PI package with a "no preview" notice and refreshes the list', async () => {
    const { calls } = await open('pi', /Packages/, /pi-review-kit/);

    await fireEvent.click(screen.getByRole('button', { name: 'Remove' }));

    const confirm = await dialog();
    expect(await within(confirm).findByText('pi remove npm:@demo/pi-review-kit@1.0.0')).toBeInTheDocument();
    expect(within(confirm).getByText('No preview available — the command will run as shown.')).toBeInTheDocument();

    await confirmWith('Remove');
    await within(await dialog()).findByText('Confirmed: the refreshed list shows the change.');

    expect(calls('extension_action').map((call) => call.dryRun)).toEqual([true, false]);

    await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    await waitFor(() => expect(screen.queryByRole('button', { name: /pi-review-kit/ })).toBeNull());
  });
});
