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
    expect(within(rowOf(table, 'assess')).queryByRole('button', { name: 'Map assess to a model' })).toBeNull();
    expect(within(rowOf(table, 'assess')).getByRole('button', { name: 'Change the model of assess' })).toBeInTheDocument();
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

describe('Changing the model of a role in OMP', () => {
  it('offers Change on every mapped row of the settings', async () => {
    const table = await openTable();

    for (const role of ['default', 'smol']) {
      expect(within(rowOf(table, role)).getAllByRole('button')).toHaveLength(2);
      expect(within(rowOf(table, role)).getByRole('button', { name: `Thinking level of ${role}: Default` })).toBeInTheDocument();
      expect(within(rowOf(table, role)).getByRole('button', { name: `Change the model of ${role}` })).toHaveTextContent('Change');
    }

    // The earlier tests mapped review in the preview backend, so it has turned into a row that Change edits too.
    expect(within(rowOf(table, 'review')).getByRole('button', { name: 'Change the model of review' })).toBeInTheDocument();
  });

  it('starts from the model the row has, saves the one chosen and shows it in the row', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const table = await openTable();

    await fireEvent.click(within(rowOf(table, 'smol')).getByRole('button', { name: 'Change the model of smol' }));
    const field = (await screen.findByRole('textbox', { name: 'Model for smol' })) as HTMLInputElement;
    expect(field.value).toBe('anthropic/claude-haiku-4-5');

    await fireEvent.click(await screen.findByRole('button', { name: 'openai/gpt-5' }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('omp_set_role', { role: 'smol', selector: 'openai/gpt-5' }));
    await waitFor(() => expect(within(rowOf(table, 'smol')).getByText('gpt-5')).toBeInTheDocument());
    expect(within(rowOf(table, 'smol')).getByText('openai/')).toBeInTheDocument();
  });

  it('says why a model could not be changed', async () => {
    vi.spyOn(ipc, 'invoke').mockRejectedValue({ code: 'conflict', message: 'The OMP settings changed' });
    renderCard();

    await fireEvent.click(screen.getByRole('button', { name: 'Change the model of default' }));
    const field = await screen.findByRole('textbox', { name: 'Model for default' });
    await fireEvent.input(field, { target: { value: 'openai-codex/gpt-6.1-sol:high' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0]).toMatchObject({ title: 'Could not change default', description: 'The OMP settings changed', tone: 'danger' });
  });

  it('keeps the roles of a project or of a broken file read-only', () => {
    const view: ConfigView = { harness: 'omp', sections: [], notices: [] };
    const section: ConfigSection = {
      id: 'user.settings',
      title: 'Settings',
      description: null,
      source: { path: '/cfg/config.yml', layer: 'user', profile: null, format: 'yaml', exists: true },
      entries: [],
      error: null,
    };

    expect(roleEditingIn(view, section)?.change).toBeTypeOf('function');
    expect(roleEditingIn(view, { ...section, id: 'profile.settings' })?.change).toBeTypeOf('function');
    expect(roleEditingIn(view, { ...section, id: 'project.settings', source: { ...section.source, layer: 'project' } })?.change).toBeUndefined();
    expect(roleEditingIn(view, { ...section, error: 'bad yaml' })?.change).toBeUndefined();
  });
});

describe('The thinking level of a role in OMP', () => {
  it('leaves a level OMP does not take as written, so no second level is put after it', () => {
    const roles: ConfigField = {
      key: 'modelRoles',
      value: {
        kind: 'map',
        value: [
          { key: 'default', value: { kind: 'text', value: 'openai/gpt-5:none' } },
          { key: 'smol', value: { kind: 'text', value: 'openai/gpt-5:low' } },
        ],
      },
    };
    const view: ConfigView = { harness: 'omp', sections: [], notices: [] };
    const section: ConfigSection = {
      id: 'user.settings',
      title: 'Settings',
      description: null,
      source: { path: '/cfg/config.yml', layer: 'user', profile: null, format: 'yaml', exists: true },
      entries: [roles],
      error: null,
    };

    render(StructuredView, { props: { fields: [roles], query: '', label: 'Settings', format: 'yaml', roleEditing: roleEditingIn(view, section) } });
    const table = screen.getByRole('table', { name: 'Model roles' });

    expect(within(rowOf(table, 'default')).queryByRole('button', { name: /^Thinking level of / })).toBeNull();
    expect(within(rowOf(table, 'default')).getByText('none')).toBeInTheDocument();
    expect(within(rowOf(table, 'smol')).getByRole('button', { name: 'Thinking level of smol: low' })).toBeInTheDocument();
  });

  it('lists the levels the model offers and saves the one chosen after the model', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const table = await openTable();

    await fireEvent.click(within(rowOf(table, 'default')).getByRole('button', { name: 'Thinking level of default: Default' }));
    const menu = await screen.findByRole('menu', { name: 'Thinking level of default' });
    await waitFor(() => expect(within(menu).getByRole('menuitemradio', { name: 'high' })).toBeInTheDocument());

    expect(within(menu).getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual(['Default', 'off', 'minimal', 'low', 'medium', 'high']);
    expect(within(menu).getByRole('menuitemradio', { name: 'Default' })).toHaveAttribute('aria-checked', 'true');

    await fireEvent.click(within(menu).getByRole('menuitemradio', { name: 'high' }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('omp_set_role', { role: 'default', selector: 'anthropic/claude-sonnet-4-5:high' }));
    await waitFor(() => expect(within(rowOf(table, 'default')).getByRole('button', { name: 'Thinking level of default: high' })).toBeInTheDocument());
  });

  it('keeps the level when the new model offers it, and starts the field from the model alone', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const table = await openTable();

    await fireEvent.click(within(rowOf(table, 'default')).getByRole('button', { name: 'Change the model of default' }));
    const field = (await screen.findByRole('textbox', { name: 'Model for default' })) as HTMLInputElement;
    expect(field.value).toBe('anthropic/claude-sonnet-4-5');
    expect(screen.getByText('Pick a model below, or type one and press Enter. It keeps high when the model offers it.')).toBeInTheDocument();

    await fireEvent.click(await screen.findByRole('button', { name: 'openai/gpt-5' }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('omp_set_role', { role: 'default', selector: 'openai/gpt-5:high' }));
  });

  it('takes a level typed with the model over the row\'s', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const table = await openTable();

    await fireEvent.click(within(rowOf(table, 'default')).getByRole('button', { name: 'Change the model of default' }));
    const field = await screen.findByRole('textbox', { name: 'Model for default' });
    await fireEvent.input(field, { target: { value: 'openai/gpt-5:low' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('omp_set_role', { role: 'default', selector: 'openai/gpt-5:low' }));
  });

  it('drops the level for a model that does not reason and says it has none', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const table = await openTable();

    await fireEvent.click(within(rowOf(table, 'default')).getByRole('button', { name: 'Change the model of default' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'ollama/qwen3-coder:30b' }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('omp_set_role', { role: 'default', selector: 'ollama/qwen3-coder:30b' }));

    await waitFor(() => expect(within(rowOf(table, 'default')).getByRole('button', { name: 'Thinking level of default: Default' })).toBeInTheDocument());
    await fireEvent.click(within(rowOf(table, 'default')).getByRole('button', { name: 'Thinking level of default: Default' }));
    expect(await screen.findByText('This model has no thinking levels.')).toBeInTheDocument();
    expect(screen.getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual(['Default']);
  });
});
