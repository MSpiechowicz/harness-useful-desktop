import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { MemoryEmbedderState } from '$lib/ipc/bindings/MemoryEmbedderState';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { IpcFailure } from '$lib/ipc/transport';
import { harness } from '$lib/stores/harness.svelte';
import { memorySearch } from '$lib/stores/memoryEmbedder.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import SettingsView from './SettingsView.svelte';

const READY: MemoryEmbedderState = { phase: 'ready', progress: null, model: 'multilingual', error: null, modelPresent: true };
/** Stopped, with the model still on this computer. */
const OFF: MemoryEmbedderState = { phase: 'off', progress: null, model: null, error: null, modelPresent: true };
/** Stopped after the model was removed. */
const OFF_WITHOUT_MODEL: MemoryEmbedderState = { ...OFF, modelPresent: false };

interface Options {
  /** The server lists neither `embedder` nor `settings`. */
  old?: boolean;
  /** A team database is configured. */
  team?: boolean;
  englishNotes?: boolean;
  removal?: () => MemoryEmbedderState;
  /** The stopped embedder the watch starts with while vector search is off. */
  stopped?: MemoryEmbedderState;
  /** What the server's feature lines say, over the mock's (a current server). */
  features?: Partial<Pick<MemoryRemote, 'embedderSupported' | 'settingsSupported' | 'notesLangSupported'>>;
  /** What the backend stores as `vectorSearch`, when it differs from what the webview loaded (a launch-time opt-out). */
  stored?: boolean;
}

/** A Harness Memory that answers the embedder and settings commands, and lets the test push embedder states the way the watch does. */
function fakeEmbedder({ old = false, team = false, englishNotes = true, removal = () => OFF_WITHOUT_MODEL, stopped = OFF, features = {}, stored: storedVector }: Options = {}) {
  const passthrough = ipc.invoke.bind(ipc);
  let emit: (state: MemoryEmbedderState) => void = () => {};
  let stored = englishNotes;

  const invoke = vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    switch (name) {
      case 'memory_embedder_watch':
        emit = (args as CommandArgs<'memory_embedder_watch'>).onEvent;
        emit((storedVector ?? prefs.value?.vectorSearch) === false ? stopped : READY);

        return null as CommandResult<K>;
      case 'prefs_get': {
        const saved = (await passthrough(name, args)) as CommandResult<'prefs_get'>;

        return (storedVector === undefined ? saved : { ...saved, vectorSearch: storedVector }) as CommandResult<K>;
      }
      case 'memory_settings_get':
      case 'memory_settings_set': {
        if (old) {
          throw IpcFailure.from({ code: 'unsupported', message: 'no settings' });
        }

        stored = name === 'memory_settings_set' ? (args as CommandArgs<'memory_settings_set'>).englishNotes : stored;

        return { englishNotes: stored } as CommandResult<K>;
      }
      case 'memory_set_vector_search': {
        const { enabled } = args as CommandArgs<'memory_set_vector_search'>;

        return { vectorSearch: enabled, state: enabled ? READY : stopped } as CommandResult<K>;
      }
      case 'memory_embedder_remove_model':
        return removal() as CommandResult<K>;
      case 'memory_remote_get': {
        const remote = (await passthrough(name, args)) as MemoryRemote;

        const lines = old ? { embedderSupported: false, settingsSupported: false } : features;
        const listed = { ...remote, ...lines };

        return (team ? { ...listed, enabled: true, kind: 'postgres', urlRedacted: 'postgres://me@db.team.example:5432/mem' } : listed) as CommandResult<K>;
      }
      default:
        return passthrough(name, args);
    }
  });

  return { invoke, push: (state: MemoryEmbedderState) => emit(state) };
}

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  Element.prototype.scrollIntoView = vi.fn();
  await harness.load();
});

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'off', vectorSearch: true });
  memoryRemote.value = null;
  memorySearch.reset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

const memorySection = (): HTMLElement => screen.getByRole('region', { name: 'Memory' });
const sent = (invoke: ReturnType<typeof fakeEmbedder>['invoke'], name: string): unknown[] => invoke.mock.calls.filter(([command]) => command === name).map(([, args]) => args);

describe('Vector search in the Memory section', () => {
  it('is on by default and says nothing more than "Ready" once the model serves', async () => {
    fakeEmbedder();
    render(SettingsView);

    const vector = await within(memorySection()).findByRole('switch', { name: 'Vector search' });

    expect(vector).toHaveAttribute('aria-checked', 'true');
    expect(await within(memorySection()).findByText('Ready')).toBeInTheDocument();
    expect(within(memorySection()).queryByRole('progressbar')).toBeNull();
  });

  it('shows the ready state as a live success dot, a readable label and a short detail', async () => {
    fakeEmbedder();
    render(SettingsView);

    const label = await within(memorySection()).findByText('Ready');
    const dot = within(memorySection()).getByTestId('vector-status-dot');

    expect(dot).toHaveClass('live-dot', 'text-success');
    expect(label).toHaveClass('text-fg');
    expect(label).not.toHaveClass('text-fg-3');
    expect(within(memorySection()).getByText('Notes are searched by meaning')).toBeInTheDocument();
  });

  it('keeps one row for every phase: an accent dot while it works, a danger dot when it stopped, no detail', async () => {
    const backend = fakeEmbedder();
    render(SettingsView);
    await within(memorySection()).findByText('Ready');

    backend.push({ phase: 'verifying', progress: null, model: null, error: null, modelPresent: false });
    expect(await within(memorySection()).findByText('Checking the search model')).toBeInTheDocument();
    expect(within(memorySection()).getByTestId('vector-status-dot')).toHaveClass('text-accent');
    expect(within(memorySection()).queryByText('Notes are searched by meaning')).toBeNull();

    backend.push({ phase: 'failed', progress: null, model: null, error: 'Boom', modelPresent: false });
    expect(await within(memorySection()).findByText('Vector search stopped')).toBeInTheDocument();
    expect(within(memorySection()).getByTestId('vector-status-dot')).toHaveClass('text-danger');

    backend.push({ phase: 'elsewhere', progress: null, model: null, error: null, modelPresent: null });
    expect(await within(memorySection()).findByText('Running in another Harness Desktop window')).toBeInTheDocument();
    expect(within(memorySection()).getByTestId('vector-status-dot')).toHaveClass('text-fg-3');
  });

  it('shows neither switch while the server lacks the feature', async () => {
    fakeEmbedder({ old: true });
    render(SettingsView);

    await within(memorySection()).findByRole('switch', { name: 'Project memory' });
    await waitFor(() => expect(memorySearch.settingsSupported).toBe(false));

    expect(within(memorySection()).queryByRole('switch', { name: 'Vector search' })).toBeNull();
    expect(within(memorySection()).queryByRole('switch', { name: 'English notes' })).toBeNull();
  });

  it('follows the server\'s feature lines for each switch, whatever a call answered', async () => {
    fakeEmbedder({ features: { embedderSupported: true, settingsSupported: false } });
    render(SettingsView);

    expect(await within(memorySection()).findByRole('switch', { name: 'Vector search' })).toBeInTheDocument();
    await waitFor(() => expect(memorySearch.settingsSupported).toBe(false));
    expect(within(memorySection()).queryByRole('switch', { name: 'English notes' })).toBeNull();
  });

  it('hides the switch for good once the server refuses it', async () => {
    const { invoke } = fakeEmbedder();
    invoke.mockImplementation(async <K extends CommandName>(name: K) => {
      if (name === 'memory_set_vector_search') {
        throw IpcFailure.from({ code: 'unsupported', message: 'no embedder' });
      }

      if (name === 'memory_settings_get') {
        return { englishNotes: true } as CommandResult<K>;
      }

      return null as CommandResult<K>;
    });
    render(SettingsView);

    await fireEvent.click(await within(memorySection()).findByRole('switch', { name: 'Vector search' }));

    await waitFor(() => expect(within(memorySection()).queryByRole('switch', { name: 'Vector search' })).toBeNull());
  });

  it('follows the watch: a bar with the percent while it downloads, a sweep while the size is unknown', async () => {
    const backend = fakeEmbedder();
    render(SettingsView);
    await within(memorySection()).findByText('Ready');

    backend.push({ phase: 'downloading', progress: 0.42, model: null, error: null, modelPresent: false });

    const bar = await within(memorySection()).findByRole('progressbar', { name: 'Vector search progress' });
    expect(bar).toHaveAttribute('aria-valuenow', '42');
    expect(within(memorySection()).getByText('Downloading the search model')).toBeInTheDocument();
    expect(within(memorySection()).getByText('42%')).toBeInTheDocument();

    backend.push({ phase: 'downloading', progress: null, model: null, error: null, modelPresent: false });

    await waitFor(() => expect(within(memorySection()).getByRole('progressbar')).not.toHaveAttribute('aria-valuenow'));
    expect(within(memorySection()).queryByText(/%$/)).toBeNull();

    backend.push({ phase: 'backfilling', progress: 0.5, model: 'multilingual', error: null, modelPresent: true });
    expect(await within(memorySection()).findByText('Indexing notes')).toBeInTheDocument();

    backend.push({ phase: 'updating', progress: 0.1, model: 'multilingual', error: null, modelPresent: true });
    expect(await within(memorySection()).findByText('Updating the search model')).toBeInTheDocument();

    backend.push(READY);
    await waitFor(() => expect(within(memorySection()).queryByRole('progressbar')).toBeNull());
  });

  it('says why it stopped without a retry button, and where it runs when another window has it', async () => {
    const backend = fakeEmbedder();
    render(SettingsView);
    await within(memorySection()).findByText('Ready');

    backend.push({ phase: 'failed', progress: null, model: null, error: 'The model could not be downloaded', modelPresent: false });

    expect(await within(memorySection()).findByText('Vector search stopped')).toBeInTheDocument();
    expect(within(memorySection()).getByText('The model could not be downloaded')).toBeInTheDocument();
    expect(within(memorySection()).queryByRole('button', { name: /retry|refresh/i })).toBeNull();

    backend.push({ phase: 'elsewhere', progress: null, model: null, error: null, modelPresent: null });

    expect(await within(memorySection()).findByText('Running in another Harness Desktop window')).toBeInTheDocument();
    expect(within(memorySection()).queryByText('Vector search stopped')).toBeNull();
  });

  it('offers to remove the downloaded model only while vector search is off, and asks inline first', async () => {
    const { invoke } = fakeEmbedder();
    render(SettingsView);

    const vector = await within(memorySection()).findByRole('switch', { name: 'Vector search' });
    expect(within(memorySection()).queryByRole('button', { name: 'Remove downloaded model' })).toBeNull();

    await fireEvent.click(vector);

    expect(sent(invoke, 'memory_set_vector_search')).toEqual([{ enabled: false }]);
    const remove = await within(memorySection()).findByRole('button', { name: 'Remove downloaded model' });
    expect(within(memorySection()).queryByText('Ready')).toBeNull();

    await fireEvent.click(remove);

    const confirm = await within(memorySection()).findByRole('alertdialog', { name: 'Remove the downloaded model?' });
    expect(sent(invoke, 'memory_embedder_remove_model')).toEqual([]);

    await fireEvent.click(within(confirm).getByRole('button', { name: 'Remove' }));

    await waitFor(() => expect(sent(invoke, 'memory_embedder_remove_model')).toHaveLength(1));
    await waitFor(() => expect(within(memorySection()).queryByRole('button', { name: 'Remove downloaded model' })).toBeNull());

    // Turning it on again downloads the model again, and turning it off later offers the removal again.
    await fireEvent.click(vector);
    await fireEvent.click(vector);

    expect(await within(memorySection()).findByRole('button', { name: 'Remove downloaded model' })).toBeInTheDocument();
  });

  it('offers the removal only once the server says a model is downloaded', async () => {
    const backend = fakeEmbedder({ stopped: { ...OFF, modelPresent: null } });
    prefs.value = samplePrefs({ motion: 'off', vectorSearch: false });
    render(SettingsView);

    await within(memorySection()).findByRole('switch', { name: 'Vector search' });
    await waitFor(() => expect(memorySearch.state?.modelPresent).toBeNull());
    expect(within(memorySection()).queryByRole('button', { name: 'Remove downloaded model' })).toBeNull();

    backend.push(OFF_WITHOUT_MODEL);
    await waitFor(() => expect(memorySearch.state?.modelPresent).toBe(false));
    expect(within(memorySection()).queryByRole('button', { name: 'Remove downloaded model' })).toBeNull();

    backend.push(OFF);
    expect(await within(memorySection()).findByRole('button', { name: 'Remove downloaded model' })).toBeInTheDocument();
  });

  it('shows the switch off and offers the removal when the backend turned vector search off after the prefs were loaded', async () => {
    fakeEmbedder({ stored: false });
    prefs.value = samplePrefs({ motion: 'off', vectorSearch: true });
    render(SettingsView);

    const vector = await within(memorySection()).findByRole('switch', { name: 'Vector search' });

    await waitFor(() => expect(vector).toHaveAttribute('aria-checked', 'false'));
    expect(await within(memorySection()).findByRole('button', { name: 'Remove downloaded model' })).toBeInTheDocument();
  });

  it('keeps the confirmation open with the reason when the removal fails', async () => {
    fakeEmbedder({
      removal: () => {
        throw IpcFailure.from({ code: 'process', message: 'The embedder is still running' });
      },
    });
    prefs.value = samplePrefs({ motion: 'off', vectorSearch: false });
    render(SettingsView);

    await fireEvent.click(await within(memorySection()).findByRole('button', { name: 'Remove downloaded model' }));
    await fireEvent.click(within(await within(memorySection()).findByRole('alertdialog')).getByRole('button', { name: 'Remove' }));

    expect(await within(memorySection()).findByText('The embedder is still running')).toBeInTheDocument();
    expect(within(memorySection()).getByRole('alertdialog')).toBeInTheDocument();
  });
});

describe('English notes in the Memory section', () => {
  it('is on by default and stores a change', async () => {
    const { invoke } = fakeEmbedder();
    render(SettingsView);

    const english = await within(memorySection()).findByRole('switch', { name: 'English notes' });
    expect(english).toHaveAttribute('aria-checked', 'true');

    await fireEvent.click(english);

    await waitFor(() => expect(sent(invoke, 'memory_settings_set')).toEqual([{ englishNotes: false }]));
    await waitFor(() => expect(english).toHaveAttribute('aria-checked', 'false'));
  });

  it('says nothing about a team database while none is configured', async () => {
    fakeEmbedder();
    render(SettingsView);
    await within(memorySection()).findByRole('switch', { name: 'English notes' });

    expect(within(memorySection()).getByText(/Your own saves are kept, with a warning\./)).toBeInTheDocument();
    expect(within(memorySection()).queryByText(/team database/, { selector: 'p' })).toBeNull();
  });

  it('says it applies to everyone who uses the team database when one is configured', async () => {
    fakeEmbedder({ team: true });
    render(SettingsView);
    await within(memorySection()).findByRole('switch', { name: 'English notes' });

    expect(await within(memorySection()).findByText(/This applies to everyone who uses the team database\./)).toBeInTheDocument();
  });
});
