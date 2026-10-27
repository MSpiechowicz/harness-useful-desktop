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
    expect(within(dialog).getAllByText(/Writes to ~\/\.omp\/agent\/mcp\.json/).length).toBe(3);

    expect(box('local-files')).not.toBeChecked();
    expect(box('time')).not.toBeChecked();
    expect(box('docs-search')).toBeChecked();
    expect(box('harness-useful-skills')).toBeChecked();

    const notices = dialog.querySelectorAll('[data-runs-program]');

    expect(notices).toHaveLength(3);
    expect(notices[0]).toHaveTextContent('Runs a program on this computer');
    expect(within(dialog).getByText('Needs GITHUB_TOKEN. Set it after importing.')).toBeInTheDocument();
    expect(within(dialog).getByText('Needs Authorization. Set it after importing.')).toBeInTheDocument();
  });

  it('cannot tick what is not new, and says why', async () => {
    const dialog = await review();

    for (const name of ['acme-tools', 'review-helper@acme-tools', 'orphan@unknown-market', 'linear']) {
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

    expect(confirm()).toHaveTextContent('Import selected (7)');

    for (const name of ['harness-useful-skills', 'docs-search', 'npm:@acme/pi-theme@1.2.0', 'opencode-wakatime@1.2.3', 'docs', 'status']) {
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
      selected: ['omp:marketplace:harness-useful-skills', 'omp:plugin:harness-useful-skills@harness-useful-skills', 'pi:package:npm:@acme/pi-theme@1.2.0', 'pi:mcp:time', 'opencode:plugin:opencode-wakatime', 'opencode:mcp:docs', 'opencode:mcp:status'],
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
    expect(within(dialog).getAllByText('Done').filter((el) => !el.closest('button'))).toHaveLength(5);
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
    expect(within(dialog).getAllByText('Waiting')).toHaveLength(6);

    await fireEvent.keyDown(dialog, { key: 'Escape' });

    expect(screen.getByRole('dialog', { name: 'Importing…' })).toBeInTheDocument();

    finish({ ok: true, items: [] });
    await waitFor(() => expect(screen.getByRole('dialog', { name: 'Import finished' })).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Close dialog' })).toBeInTheDocument();
  });

  describe('OpenCode', () => {
    it('lists a plugin as name@version, installed as pinned, with no marketplace and no version note', async () => {
      const dialog = await review();
      const row = within(box('opencode-wakatime@1.2.3').closest('li') as HTMLElement);

      expect(row.getByText('Plugin')).toBeInTheDocument();
      expect(row.getByText('opencode-wakatime@1.2.3')).toBeInTheDocument();
      expect(row.getByText('opencode plugin opencode-wakatime@1.2.3 -g')).toBeInTheDocument();
      expect(row.queryByText(/Shared version/)).not.toBeInTheDocument();
      expect(row.queryByText(/latest one is installed/)).not.toBeInTheDocument();
      expect(row.queryByText(/marketplace/i)).not.toBeInTheDocument();
      expect(within(dialog).getByText('opencode-notify@0.4.0')).toBeInTheDocument();
    });

    it('shows the entry written to the OpenCode file, in the shape the file uses', async () => {
      await review();

      const local = within(box('repo-search').closest('li') as HTMLElement);

      expect(local.getByText(/Writes to ~\/\.config\/opencode\/opencode\.json/)).toBeInTheDocument();
      expect(local.getByText(/"type": "local",\s+"command": \["npx", "-y", "@acme\/repo-search-mcp"\],\s+"enabled": false/)).toBeInTheDocument();
      expect(local.getByText('Needs REPO_TOKEN. Set it after importing.')).toBeInTheDocument();
      expect(local.queryByText(/mcpServers|"args"/)).not.toBeInTheDocument();

      const remote = within(box('status').closest('li') as HTMLElement);

      expect(remote.getByText(/"type": "remote",\s+"url": "https:\/\/status\.example\.com\/mcp",\s+"enabled": true/)).toBeInTheDocument();
    });

    it('notes on each selectable new server that it is checked when importing, and on nothing else', async () => {
      const dialog = await review();
      const note = /Checked against OpenCode’s merged configuration when you import\. The check starts OpenCode once\./;

      for (const name of ['repo-search', 'docs', 'status']) {
        expect(within(box(name).closest('li') as HTMLElement).getByText(note)).toBeInTheDocument();
      }

      expect(within(dialog).getAllByText(note)).toHaveLength(3);

      const writtenDisabled = within(box('docs').closest('li') as HTMLElement).getAllByRole('listitem').map((item) => item.textContent);

      expect(writtenDisabled.at(-2)).toMatch(/Written disabled/);
      expect(writtenDisabled.at(-1)).toMatch(/Checked against OpenCode/);
      expect(within(box('opencode-wakatime@1.2.3').closest('li') as HTMLElement).queryByText(note)).not.toBeInTheDocument();
      expect(within(dialog).queryByText('opencode-notify@0.4.0')?.closest('li')).not.toHaveTextContent('Checked against');
    });

    it('shows a server OpenCode already defines as a conflict that is left alone', async () => {
      useScenario('exists');
      await review();
      await fireEvent.click(box('status'));
      await fireEvent.click(box('opencode-wakatime@1.2.3'));
      await fireEvent.click(screen.getByRole('button', { name: /^Import selected/ }));

      const dialog = await screen.findByRole('dialog', { name: 'Import finished' });
      const row = within(within(dialog).getByText('docs').closest('li') as HTMLElement);

      expect(row.getByText('Conflict')).toBeInTheDocument();
      expect(row.getByText('Something with this name but a different definition exists here. It is left alone.')).toBeInTheDocument();
      expect(within(dialog).queryByText('Failed')).not.toBeInTheDocument();
      expect(within(dialog).getByText('Some items were not set up. What worked stays; nothing is rolled back.')).toBeInTheDocument();
    });

    it('says a config file could not be read on the review row instead of calling it a different definition', async () => {
      useScenario('unreadable');
      await review();

      const row = within(screen.getByText('status').closest('li') as HTMLElement);

      expect(row.getByText('Conflict')).toBeInTheDocument();
      expect(row.getByText(/A config file could not be read, so the app can’t tell whether this server already exists\. Nothing is written\./)).toBeInTheDocument();
      expect(row.queryByText(/a different definition/)).not.toBeInTheDocument();
    });

    it('says it on the result row too when the check could not read a config file', async () => {
      useScenario('unreadable');
      await review();
      await fireEvent.click(screen.getByRole('button', { name: /^Import selected/ }));

      const dialog = await screen.findByRole('dialog', { name: 'Import finished' });
      const row = within(within(dialog).getByText('docs').closest('li') as HTMLElement);

      expect(row.getByText('Conflict')).toBeInTheDocument();
      expect(row.getByText(/A config file could not be read, so the app can’t tell whether this server already exists\. Nothing is written\./)).toBeInTheDocument();
      expect(row.queryByText(/a different definition/)).not.toBeInTheDocument();
    });

    it('hands over the entry to paste when the file has comments, as a note and not an error', async () => {
      useScenario('commented');
      await review();
      await fireEvent.click(box('docs'));
      await fireEvent.click(box('status'));
      await fireEvent.click(box('opencode-wakatime@1.2.3'));
      await fireEvent.click(box('repo-search'));
      await fireEvent.click(screen.getByRole('button', { name: /^Import selected/ }));

      const dialog = await screen.findByRole('dialog', { name: 'Import finished' });
      const row = within(within(dialog).getByText('repo-search').closest('li') as HTMLElement);

      expect(row.getByText('Skipped')).toBeInTheDocument();
      expect(row.getByText(/~\/\.config\/opencode\/opencode\.jsonc has comments that rewriting would lose, so nothing was written there/)).toBeInTheDocument();

      const snippet = (row.getByRole('button', { name: 'Copy snippet' }).closest('div')?.textContent ?? '').replace(/\s+/g, '');

      expect(snippet).toContain('"repo-search":{"type":"local","command":["npx","-y","@acme/repo-search-mcp"],"enabled":false}');
      expect(row.getByRole('button', { name: 'Copy snippet' })).toBeInTheDocument();
      expect(within(dialog).queryByRole('alert')).not.toBeInTheDocument();
      expect(within(dialog).queryByText('Failed')).not.toBeInTheDocument();
      expect(within(dialog).getByText('Everything else is in place. Add the entries below to their files yourself.')).toBeInTheDocument();
    });
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
