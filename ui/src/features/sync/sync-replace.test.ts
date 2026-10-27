import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { SyncItemOutcome } from '$lib/ipc/bindings/SyncItemOutcome';
import { ipc } from '$lib/ipc/client';
import { defaultScenario } from '$lib/ipc/mock/scenarios';
import { createMockTransport } from '$lib/ipc/mock/transport';

import { freshBackend, type Backend } from '../../../tests/ecosystem-backend';
import SyncSection from './SyncSection.svelte';

let backend: Backend;

beforeEach(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  window.history.replaceState(null, '', '/?sync=typical');
  backend = freshBackend();
});

afterEach(() => {
  window.history.replaceState(null, '', '/');
  vi.restoreAllMocks();
});

const replaceBox = () => screen.getByRole('checkbox', { name: 'Replace github' });
const rowOf = (element: HTMLElement) => within(element.closest('li') as HTMLElement);

/** Pastes a code, reviews it and returns the review dialog. */
async function review(): Promise<HTMLElement> {
  render(SyncSection, { id: 'sync' });
  await fireEvent.click(screen.getByRole('button', { name: 'Import…' }));
  await screen.findByRole('dialog', { name: 'Import setup' });
  await fireEvent.input(screen.getByRole('textbox', { name: 'Setup code or JSON' }), { target: { value: 'hud1:abc' } });
  await fireEvent.click(screen.getByRole('button', { name: 'Review' }));

  return screen.findByRole('dialog', { name: 'Review the import' });
}

describe('Replacing an MCP server that has another definition', () => {
  it('offers it unticked, and says what is replaced and what is kept', async () => {
    await review();

    expect(replaceBox()).toBeEnabled();
    expect(replaceBox()).not.toBeChecked();

    const row = rowOf(replaceBox());

    expect(row.getByText('Different')).toBeInTheDocument();
    expect(row.getByText(/A server with this name but another definition is here\. Tick it to replace it\. Its saved values \(env, headers, on\/off\) are kept only while the address of a remote server stays the same\. A local program is always rewritten without them\./)).toBeInTheDocument();
    expect(row.getByText('Needs Authorization. Values already set here are kept.')).toBeInTheDocument();
    expect(row.getByText(/Replaces in ~\/\.omp\/agent\/mcp\.json/)).toBeInTheDocument();
    expect(row.getByText(/"type": "http",\s+"url": "https:\/\/api\.githubcopilot\.com\/mcp\/"/)).toBeInTheDocument();
    expect(row.queryByText(/Writes to/)).not.toBeInTheDocument();
    expect(row.queryByText(/enabled/)).not.toBeInTheDocument();
    expect(row.queryByText(/Set it after importing/)).not.toBeInTheDocument();
  });

  it('writes the whole entry, like a new server, when its address changes', async () => {
    await review();

    const box = screen.getByRole('checkbox', { name: 'Replace sentry' });

    expect(box).toBeEnabled();
    expect(box).not.toBeChecked();

    const row = rowOf(box);

    expect(row.getByText('Different')).toBeInTheDocument();
    expect(row.getByText(/Writes to ~\/\.omp\/agent\/mcp\.json/)).toBeInTheDocument();
    expect(row.getByText(/"type": "http",\s+"url": "https:\/\/mcp\.sentry\.example\/mcp",\s+"enabled": false/)).toBeInTheDocument();
    expect(row.queryByText(/Replaces in/)).not.toBeInTheDocument();
    expect(row.getByText('Needs X-Api-Key. Set it after importing.')).toBeInTheDocument();
    expect(row.queryByText(/Values already set here are kept/)).not.toBeInTheDocument();
    expect(row.getByText(/Written disabled/)).toBeInTheDocument();
  });

  it('keeps a server the app cannot replace as a conflict that cannot be ticked', async () => {
    await review();

    expect(screen.getByRole('checkbox', { name: 'Import linear' })).toBeDisabled();
    expect(screen.queryByRole('checkbox', { name: 'Replace linear' })).not.toBeInTheDocument();
    expect(rowOf(screen.getByRole('checkbox', { name: 'Import linear' })).getByText('Conflict')).toBeInTheDocument();
  });

  it('is not part of the import until it is ticked, and then sends its id and shows Replaced', async () => {
    await review();

    expect(screen.getByRole('button', { name: /^Import selected/ })).toHaveTextContent('Import selected (7)');

    await fireEvent.click(replaceBox());

    expect(replaceBox()).toBeChecked();
    expect(screen.getByRole('button', { name: /^Import selected/ })).toHaveTextContent('Import selected (8)');

    await fireEvent.click(screen.getByRole('button', { name: /^Import selected/ }));

    const dialog = await screen.findByRole('dialog', { name: 'Import finished' });
    const [call] = backend.calls('sync_import_apply');

    expect(call).toMatchObject({ confirmed: true, selected: expect.arrayContaining(['omp:mcp:github']) });

    const row = rowOf(within(dialog).getByText('github'));

    expect(row.getByText('Replaced')).toBeInTheDocument();
    expect(within(dialog).getByText('Everything you selected is in place.')).toBeInTheDocument();
  });

  it('hands over the fields to replace when the file has comments, as a note and not an error', async () => {
    const scenario = defaultScenario();

    scenario.handlers.sync_import_apply = async ({ onProgress }) => {
      const outcome: SyncItemOutcome = {
        id: 'omp:mcp:github',
        harness: 'omp',
        kind: 'mcpServer',
        label: 'github',
        outcome: { type: 'skipped', reason: { type: 'manualReplace', file: '~/.omp/agent/mcp.json', snippet: '"type": "http",\n"url": "https://api.githubcopilot.com/mcp/"' } },
        summary: null,
        argv: [],
        notes: [],
      };

      onProgress({ type: 'finished', outcome });

      return { ok: false, items: [outcome] };
    };

    const transport = createMockTransport(scenario, 0);
    vi.spyOn(ipc, 'invoke').mockImplementation((name, args) => transport.invoke(name, args));

    await review();
    await fireEvent.click(replaceBox());
    await fireEvent.click(screen.getByRole('button', { name: /^Import selected/ }));

    const dialog = await screen.findByRole('dialog', { name: 'Import finished' });
    const row = rowOf(within(dialog).getByText('github'));

    expect(row.getByText('Skipped')).toBeInTheDocument();
    expect(row.getByText(/~\/\.omp\/agent\/mcp\.json has comments that rewriting would lose, so nothing was changed there\. In this server’s entry, replace these fields yourself and keep the others:/)).toBeInTheDocument();
    expect(row.getByRole('button', { name: 'Copy snippet' })).toBeInTheDocument();
    expect(within(dialog).queryByRole('alert')).not.toBeInTheDocument();
    expect(within(dialog).queryByText('Failed')).not.toBeInTheDocument();
    expect(within(dialog).getByText('Everything else is in place. Add the entries below to their files yourself.')).toBeInTheDocument();
  });
});
