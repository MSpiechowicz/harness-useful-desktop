import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import type { SyncImportResult } from '$lib/ipc/bindings/SyncImportResult';
import { defaultScenario } from '$lib/ipc/mock/scenarios';
import { createMockTransport } from '$lib/ipc/mock/transport';

import { freshBackend, type Backend } from '../../../tests/ecosystem-backend';
import SyncSection from './SyncSection.svelte';

let backend: Backend;

function useScenario(name: string): void {
  window.history.replaceState(null, '', `/?sync=${name}`);
  backend = freshBackend();
}

beforeEach(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  useScenario('typical');
});

afterEach(() => {
  window.history.replaceState(null, '', '/');
  vi.restoreAllMocks();
});

const box = (name: string) => screen.getByRole('checkbox', { name: `Import ${name}` });

async function openImport(): Promise<HTMLElement> {
  render(SyncSection, { id: 'sync' });
  await fireEvent.click(screen.getByRole('button', { name: 'Import…' }));

  return screen.findByRole('dialog', { name: 'Import setup' });
}

async function paste(text: string): Promise<void> {
  await fireEvent.input(screen.getByRole('textbox', { name: 'Setup code or JSON' }), { target: { value: text } });
}

/** Pastes a code and reviews it. */
async function review(): Promise<HTMLElement> {
  await openImport();
  await paste('hud1:abc');
  await fireEvent.click(screen.getByRole('button', { name: 'Review' }));

  return screen.findByRole('dialog', { name: 'Review the import' });
}

describe('Import', () => {
  it('never reads a pasted code by itself: only the Review button does', async () => {
    await openImport();

    expect(screen.getByRole('button', { name: 'Review' })).toBeDisabled();

    await paste('hud1:abc');

    expect(backend.calls('sync_import_preview')).toEqual([]);

    await fireEvent.click(screen.getByRole('button', { name: 'Review' }));
    await screen.findByRole('dialog', { name: 'Review the import' });

    expect(backend.calls('sync_import_preview')).toEqual([{ input: { type: 'text', text: 'hud1:abc' } }]);
  });

  it('asks Rust to open the picker for a file, and says so when none was chosen', async () => {
    useScenario('cancelled');
    await openImport();

    await fireEvent.click(screen.getByRole('button', { name: 'Open file…' }));

    expect(await screen.findByText('No file was chosen.')).toBeInTheDocument();
    expect(backend.calls('sync_import_preview')).toEqual([{ input: { type: 'file' } }]);
    expect(screen.getByRole('dialog', { name: 'Import setup' })).toBeInTheDocument();
  });

  it('shows the backend message in the paste area when the input is refused', async () => {
    await openImport();
    await paste('not a setup');
    await fireEvent.click(screen.getByRole('button', { name: 'Review' }));

    const alert = await screen.findByRole('alert');

    expect(alert).toHaveTextContent('This is not a Harness Desktop setup');
    expect(screen.getByRole('textbox', { name: 'Setup code or JSON' })).toHaveAttribute('aria-invalid', 'true');
    expect(screen.getByRole('textbox', { name: 'Setup code or JSON' })).toHaveAttribute('aria-describedby', alert.id);
  });

  it('tells that the setup is from a newer version', async () => {
    useScenario('newer');
    await openImport();
    await paste('hud1:abc');
    await fireEvent.click(screen.getByRole('button', { name: 'Review' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('made by a newer version of Harness Desktop');
  });

  it('lists the exact steps of each item, and a program server is unticked and highlighted', async () => {
    const dialog = await review();

    expect(within(dialog).getByText('omp plugin marketplace add MSpiechowicz/harness-useful-skills')).toBeInTheDocument();
    expect(within(dialog).getAllByText(/Writes to ~\/\.omp\/agent\/mcp\.json/).length).toBe(2);

    expect(box('local-files')).not.toBeChecked();
    expect(box('time')).not.toBeChecked();
    expect(box('docs-search')).toBeChecked();
    expect(box('harness-useful-skills')).toBeChecked();

    const notices = dialog.querySelectorAll('[data-runs-program]');

    expect(notices).toHaveLength(2);
    expect(notices[0]).toHaveTextContent('Runs a program on this computer');
    expect(within(dialog).getByText('Needs GITHUB_TOKEN. Set it after importing.')).toBeInTheDocument();
    expect(within(dialog).getByText('Needs Authorization. Set it after importing.')).toBeInTheDocument();
  });

  it('cannot tick what is not new, and says why', async () => {
    const dialog = await review();

    for (const name of ['acme-tools', 'review-helper@acme-tools', 'orphan@unknown-market', 'github']) {
      expect(box(name)).toBeDisabled();
      expect(box(name)).not.toBeChecked();
    }

    expect(within(dialog).getByText('Installed 1.2.0, shared 1.4.0. It is left as it is.')).toBeInTheDocument();
    expect(within(dialog).getByText(/Something with this name but a different definition/)).toBeInTheDocument();
    expect(within(dialog).getByText('Its marketplace is not registered here and not part of this setup.')).toBeInTheDocument();
  });

  it('unticks the plugins of a marketplace with it, and restores them when it is ticked again', async () => {
    await review();
    const plugin = 'harness-useful-skills@harness-useful-skills';

    expect(box(plugin)).toBeChecked();

    await fireEvent.click(box('harness-useful-skills'));

    expect(box('harness-useful-skills')).not.toBeChecked();
    expect(box(plugin)).not.toBeChecked();
    expect(box(plugin)).toBeDisabled();

    await fireEvent.click(box('harness-useful-skills'));

    expect(box(plugin)).toBeChecked();
    expect(box(plugin)).toBeEnabled();
  });

  it('does not restore a plugin the user had unticked themselves', async () => {
    await review();
    const plugin = 'harness-useful-skills@harness-useful-skills';

    await fireEvent.click(box(plugin));
    await fireEvent.click(box('harness-useful-skills'));
    await fireEvent.click(box('harness-useful-skills'));

    expect(box(plugin)).not.toBeChecked();
    expect(box(plugin)).toBeEnabled();
  });

  it('cannot be confirmed with nothing selected', async () => {
    await review();
    const confirm = () => screen.getByRole('button', { name: /^Import selected/ });

    expect(confirm()).toHaveTextContent('Import selected (4)');

    for (const name of ['harness-useful-skills', 'docs-search', 'npm:@acme/pi-theme@1.2.0']) {
      await fireEvent.click(box(name));
    }

    expect(confirm()).toHaveTextContent('Import selected (0)');
    expect(confirm()).toBeDisabled();
  });

  it('applies with confirmed: true and exactly the ticked ids of the reviewed plan', async () => {
    await review();
    await fireEvent.click(box('docs-search'));
    await fireEvent.click(box('time'));
    await fireEvent.click(screen.getByRole('button', { name: /^Import selected/ }));

    await screen.findByRole('dialog', { name: 'Import finished' });

    const [call] = backend.calls('sync_import_apply');

    expect(call).toMatchObject({
      planId: 'plan-1',
      confirmed: true,
      selected: ['omp:marketplace:harness-useful-skills', 'omp:plugin:harness-useful-skills@harness-useful-skills', 'pi:package:npm:@acme/pi-theme@1.2.0', 'pi:mcp:time'],
    });
    expect(backend.calls('sync_import_apply')).toHaveLength(1);
  });

  it('shows what each item did, and keeps going after a failure', async () => {
    useScenario('partial');
    await review();
    await fireEvent.click(screen.getByRole('button', { name: /^Import selected/ }));

    const dialog = await screen.findByRole('dialog', { name: 'Import finished' });

    expect(within(dialog).getByText('Some items were not set up. What worked stays; nothing is rolled back.')).toBeInTheDocument();
    expect(within(dialog).getByText('Failed')).toBeInTheDocument();
    expect(within(dialog).getByText('Could not clone the repository: exit code 128.')).toBeInTheDocument();
    expect(within(dialog).getByText('omp plugin marketplace add MSpiechowicz/harness-useful-skills')).toBeInTheDocument();
    expect(within(dialog).getByText('Skipped')).toBeInTheDocument();
    expect(within(dialog).getByText('Its marketplace failed.')).toBeInTheDocument();
    expect(within(dialog).getAllByText('Done', { selector: 'span' })).toHaveLength(2);
  });

  it('says so when every selected item is in place', async () => {
    await review();
    await fireEvent.click(screen.getByRole('button', { name: /^Import selected/ }));

    expect(await screen.findByText('Everything you selected is in place.')).toBeInTheDocument();
  });

  it('cannot be left or changed while it runs', async () => {
    const scenario = defaultScenario();
    let finish: (result: SyncImportResult) => void = () => {};

    scenario.handlers.sync_import_apply = (args) => {
      args.onProgress({ type: 'started', id: args.selected[0], index: 0, total: args.selected.length });

      return new Promise<SyncImportResult>((resolve) => (finish = resolve));
    };

    const transport = createMockTransport(scenario, 0);
    vi.spyOn(ipc, 'invoke').mockImplementation((name, args) => transport.invoke(name, args));

    await review();
    await fireEvent.click(screen.getByRole('button', { name: /^Import selected/ }));

    const dialog = await screen.findByRole('dialog', { name: 'Importing…' });

    expect(within(dialog).queryByRole('button', { name: 'Close dialog' })).not.toBeInTheDocument();
    await waitFor(() => expect(within(dialog).getByText('Running')).toBeInTheDocument());
    expect(within(dialog).getAllByText('Waiting')).toHaveLength(3);

    await fireEvent.keyDown(dialog, { key: 'Escape' });

    expect(screen.getByRole('dialog', { name: 'Importing…' })).toBeInTheDocument();

    finish({ ok: true, items: [] });
    await waitFor(() => expect(screen.getByRole('dialog', { name: 'Import finished' })).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Close dialog' })).toBeInTheDocument();
  });

  it('lists what it could not use: invalid items, unknown harnesses, a harness that is not installed', async () => {
    useScenario('hostile');
    const dialog = await review();

    expect(within(dialog).getByText('Not installed here')).toBeInTheDocument();
    expect(within(dialog).getByText('A value contains unsafe characters.')).toBeInTheDocument();
    expect(within(dialog).getByText('It starts a shell, which is not allowed.')).toBeInTheDocument();
    expect(within(dialog).getByText('Unknown harness “zed”, items: 4')).toBeInTheDocument();
    expect(within(dialog).getByText('Items that are not valid: 5')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^Import selected/ })).toBeDisabled();
  });
});
