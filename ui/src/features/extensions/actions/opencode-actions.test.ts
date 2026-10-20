import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

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
  cleanup();
  actionRunner.close();
  project.active = null;
  vi.restoreAllMocks();
});

/** OpenCode's extensions view, with the plugin of that name selected when one is given. */
async function openCode(plugin?: RegExp) {
  const backend = freshBackend();
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'opencode' });
  render(ExtensionsViewer);

  const group = await screen.findByRole('region', { name: /Plugins/ });
  if (plugin) {
    await fireEvent.click(within(group).getByRole('button', { name: plugin }));
  }

  return backend;
}

const dialog = () => screen.findByRole('dialog');

async function openInstall(): Promise<HTMLElement> {
  await fireEvent.click(screen.getByRole('button', { name: 'Install' }));
  await fireEvent.click(await screen.findByRole('menuitem', { name: 'Install plugin' }));

  return dialog();
}

describe('OpenCode plugin actions', () => {
  it('offers to install a plugin but no marketplace, package, or bulk update', async () => {
    await openCode(/@acme\/opencode-plugin/);

    await fireEvent.click(screen.getByRole('button', { name: 'Install' }));

    expect(await screen.findByRole('menuitem', { name: 'Install plugin' })).toBeInTheDocument();
    expect(screen.queryByRole('menuitem', { name: /marketplace|package/i })).toBeNull();
    // Nothing is left for the more-actions menu: no bulk update, and this plugin has no file to reveal.
    expect(screen.queryByRole('button', { name: 'More actions' })).toBeNull();
  });

  it('lists the removal alone on an installed plugin, with no enable, disable, or update', async () => {
    await openCode(/@acme\/opencode-plugin/);

    expect(await screen.findByRole('button', { name: 'Uninstall' })).toBeInTheDocument();
    for (const name of ['Enable', 'Disable', 'Update']) {
      expect(screen.queryByRole('button', { name })).toBeNull();
    }
  });

  it('explains a package it cannot install before anything is sent, and accepts an npm spec', async () => {
    const { calls } = await openCode();
    const install = await openInstall();

    expect(within(install).queryByText('Install for')).toBeNull();

    const input = within(install).getByRole('textbox', { name: /Plugin/ });
    for (const [value, problem] of [
      ['file:///x', /npm package name/],
      ['github:a/b', /npm package name/],
      ['a b', /Use only letters/],
      ['-g', /must not start with a dash/],
    ] as const) {
      await fireEvent.input(input, { target: { value } });
      await fireEvent.click(within(install).getByRole('button', { name: 'Review' }));

      expect(await within(install).findByText(problem), value).toBeInTheDocument();
    }
    expect(calls('extension_action')).toHaveLength(0);

    await fireEvent.input(input, { target: { value: '@scope/name@1.2.3' } });
    await fireEvent.click(within(install).getByRole('button', { name: 'Review' }));

    expect(await screen.findByText('opencode plugin @scope/name@1.2.3 -g')).toBeInTheDocument();
    expect(calls('extension_action')[0]).toMatchObject({ dryRun: true, action: { type: 'pluginInstall', target: '@scope/name@1.2.3', scope: 'user' } });
  });

  it('removes a plugin by editing the config file and updates the list', async () => {
    const { calls } = await openCode(/@acme\/opencode-plugin/);

    await fireEvent.click(await screen.findByRole('button', { name: 'Uninstall' }));

    const confirm = await dialog();
    expect(await within(confirm).findByText(/Would remove @acme\/opencode-plugin from the plugin list/)).toBeInTheDocument();
    expect(within(confirm).getByRole('region', { name: 'Config file' })).toHaveTextContent('~/.config/opencode/opencode.json');
    expect(within(confirm).queryByRole('region', { name: 'Command' })).toBeNull();
    expect(within(confirm).queryByText(/No preview available/)).toBeNull();

    const button = within(confirm).getByRole('button', { name: 'Uninstall' });
    await waitFor(() => expect(button).toBeEnabled());
    await fireEvent.click(button);

    await within(await dialog()).findByText('Confirmed: the refreshed list shows the change.');
    expect(calls('extension_action').map((call) => call.dryRun)).toEqual([true, false]);

    await fireEvent.click(within(await dialog()).getByRole('button', { name: 'Close' }));

    await waitFor(() => expect(within(screen.getByRole('region', { name: /Plugins/ })).queryByRole('button', { name: /@acme\/opencode-plugin/ })).toBeNull());
  });

  it('shows a plugin it leaves to the user as a note with the file, not as an error', async () => {
    await openCode(/hello/);

    await fireEvent.click(await screen.findByRole('button', { name: 'Uninstall' }));

    const note = await dialog();
    expect(await within(note).findByText('Edit this file yourself')).toBeInTheDocument();
    expect(within(note).getByRole('region', { name: 'Config file' })).toHaveTextContent('~/.config/opencode/plugins/hello.ts');
    expect(within(note).getByText(/hello is defined in ~\/\.config\/opencode\/plugins\/hello\.ts/)).toBeInTheDocument();
    expect(within(note).queryByRole('alert')).toBeNull();
    expect(within(note).queryByText('The preview reports a problem, so this would fail too')).toBeNull();
    expect(within(note).queryByRole('button', { name: 'Uninstall' })).toBeNull();

    await fireEvent.click(within(note).getByRole('button', { name: 'Close' }));
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
