import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';
import type { ConfigView } from '$lib/ipc/bindings/ConfigView';
import { ipc } from '$lib/ipc/client';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { configView } from '$lib/stores/views.svelte';

import { samplePrefs } from '../../../tests/helpers';
import ConfigViewer from './ConfigViewer.svelte';
import { roleEditingIn, unmappedIn } from './role-edit';
import StructuredView from './StructuredView.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp' });
  await harness.load();
});

afterEach(() => {
  vi.restoreAllMocks();
  configView.sync(null, null);
  toasts.items = [];
});

/** The card on its own, so a role stays unmapped whatever the earlier tests mapped in the preview backend. */
function renderCard(): void {
  const roles: ConfigField = { key: 'modelRoles', value: { kind: 'map', value: [{ key: 'default', value: { kind: 'text', value: 'anthropic/claude-sonnet-4-5' } }] } };

  const view: ConfigView = { harness: 'omp', sections: [], notices: [], unmappedRoles: [{ role: 'assess', inheritsSessionModel: true }] };
  const section: ConfigSection = {
    id: 'user.settings',
    title: 'Settings',
    description: null,
    source: { path: '/cfg/config.yml', layer: 'user', profile: null, format: 'yaml', exists: true },
    entries: [roles],
    error: null,
  };

  render(StructuredView, { props: { fields: [roles], query: '', label: 'Settings', format: 'yaml', roleEditing: roleEditingIn(view, section) } });
}

const rowOf = (table: HTMLElement, role: string): HTMLElement => {
  const header = within(table).getByRole('rowheader', { name: role });

  return header.closest('tr') as HTMLElement;
};

async function openTable(): Promise<HTMLElement> {
  render(ConfigViewer);

  return await screen.findByRole('table', { name: 'Model roles' });
}

// The first three tests share the preview backend, which keeps what they map: they run in this order.
describe('Mapping a Harness Skills role in OMP', () => {
  it('lists the roles the settings do not map after the mapped ones, saying what they use meanwhile', async () => {
    const table = await openTable();
    const rows = within(table).getAllByRole('row').slice(1);

    expect(rows.map((row) => within(row).getAllByRole('rowheader')[0].textContent?.trim())).toEqual(['default', 'smol', 'review', 'assess']);
    expect(within(rowOf(table, 'review')).getByText('Not mapped · uses default')).toBeInTheDocument();
    expect(within(rowOf(table, 'assess')).getByText('Not mapped · uses the session model')).toBeInTheDocument();
    expect(within(rowOf(table, 'assess')).getByRole('button', { name: 'Map assess to a model' })).toBeInTheDocument();
    expect(within(rowOf(table, 'assess')).queryByText('—')).toBeNull();
  });

  it('puts Map in the right-aligned actions column, like Change, and leaves the Thinking cell empty', async () => {
    const table = await openTable();
    const rows = within(table).getAllByRole('row');
    const cells = (row: HTMLElement) => within(row).getAllByRole('cell');
    const unmapped = cells(rowOf(table, 'assess'));

    expect(within(rows[0]).getAllByRole('columnheader')).toHaveLength(4);
    expect(within(rows[0]).getByText('Actions')).toBeInTheDocument();

    for (const row of rows.slice(1)) {
      expect(cells(row)).toHaveLength(3);
    }

    expect(unmapped[0]).toHaveTextContent('Not mapped · uses the session model');
    expect(unmapped[1]).toBeEmptyDOMElement();
    expect(unmapped[2]).toContainElement(within(rowOf(table, 'assess')).getByRole('button', { name: 'Map assess to a model' }));
    expect(unmapped[2].firstElementChild).toHaveClass('justify-end');
  });

  it('maps a role to the typed model and turns its row into a normal one', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const table = await openTable();

    await fireEvent.click(within(rowOf(table, 'assess')).getByRole('button', { name: 'Map assess to a model' }));
    const field = await screen.findByRole('textbox', { name: 'Model for assess' });
    await fireEvent.input(field, { target: { value: 'openai-codex/gpt-6.1-sol:high' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    await waitFor(() => expect(within(rowOf(table, 'assess')).getByText('gpt-6.1-sol')).toBeInTheDocument());
    expect(invoke).toHaveBeenCalledWith('omp_map_role', { role: 'assess', selector: 'openai-codex/gpt-6.1-sol:high' });
    expect(within(rowOf(table, 'assess')).getByText('high')).toBeInTheDocument();
    expect(within(rowOf(table, 'assess')).queryByRole('button')).toBeNull();
    expect(within(rowOf(table, 'review')).getByText('Not mapped · uses default')).toBeInTheDocument();
  });

  it('offers the models the other roles use and maps the one chosen', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const table = await openTable();

    await fireEvent.click(within(rowOf(table, 'review')).getByRole('button', { name: 'Map review to a model' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'anthropic/claude-haiku-4-5' }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('omp_map_role', { role: 'review', selector: 'anthropic/claude-haiku-4-5' }));
    await waitFor(() => expect(within(rowOf(table, 'review')).getByText('claude-haiku-4-5')).toBeInTheDocument());
  });

  it('offers every model the harness has, asked of it while the picker is open', async () => {
    renderCard();

    await fireEvent.click(screen.getByRole('button', { name: 'Map assess to a model' }));

    expect(await screen.findByRole('button', { name: 'openai/gpt-5' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'anthropic/claude-sonnet-4-5' })).toBeInTheDocument();
  });

  it('does not ask the backend for a model that is no provider and model', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    renderCard();

    await fireEvent.click(screen.getByRole('button', { name: 'Map assess to a model' }));
    const field = await screen.findByRole('textbox', { name: 'Model for assess' });
    await fireEvent.input(field, { target: { value: 'just-a-name' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    expect(screen.getByText('Use provider/model or provider/model:thinking.')).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith('omp_map_role', expect.anything());
  });

  it('says why a role could not be mapped and keeps its row', async () => {
    vi.spyOn(ipc, 'invoke').mockRejectedValue({ code: 'conflict', message: 'The OMP settings changed' });
    renderCard();

    await fireEvent.click(screen.getByRole('button', { name: 'Map assess to a model' }));
    const field = await screen.findByRole('textbox', { name: 'Model for assess' });
    await fireEvent.input(field, { target: { value: 'openai-codex/gpt-6.1-sol:high' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0]).toMatchObject({ title: 'Could not map assess', description: 'The OMP settings changed', tone: 'danger' });
    expect(screen.getByText('Not mapped · uses the session model')).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: 'Model for assess' })).toBeInTheDocument();
  });

  it('offers roles only in the OMP settings', () => {
    const view: ConfigView = { harness: 'omp', sections: [], notices: [], unmappedRoles: [{ role: 'assess', inheritsSessionModel: true }] };

    expect(unmappedIn(view, 'user.settings')).toHaveLength(1);
    expect(unmappedIn(view, 'profile.settings')).toHaveLength(1);
    expect(unmappedIn(view, 'user.mcp')).toEqual([]);
    expect(unmappedIn({ ...view, harness: 'claude' }, 'user.settings')).toEqual([]);
    expect(unmappedIn({ harness: 'omp', sections: [], notices: [] }, 'user.settings')).toEqual([]);
  });
});
