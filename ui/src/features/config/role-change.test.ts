import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import type { ModelOption } from '$lib/ipc/bindings/ModelOption';
import { ipc } from '$lib/ipc/client';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';
import { configView } from '$lib/stores/views.svelte';

import { samplePrefs } from '../../../tests/helpers';
import ConfigViewer from './ConfigViewer.svelte';
import { joinSelector, modelToAsk, selectorRules, splitSelector } from './role-selectors';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'claude' });
  await harness.load();
  harness.summaries = harness.summaries.map((summary) =>
    summary.info.id === 'claude' ? { ...summary, detection: { ...summary.detection, installed: true, configDirExists: true } } : summary,
  );
});

afterEach(() => {
  vi.restoreAllMocks();
  configView.sync(null, null);
  viewState.configSection = null;
  toasts.items = [];
});

const rowOf = (table: HTMLElement, role: string): HTMLElement => within(table).getByRole('rowheader', { name: role }).closest('tr') as HTMLElement;

async function openRoles(): Promise<HTMLElement> {
  viewState.configSection = 'user.harnessSkillsModels';
  render(ConfigViewer);

  return await screen.findByRole('table', { name: 'Model roles' });
}

async function openChange(table: HTMLElement, role: string): Promise<HTMLInputElement> {
  await fireEvent.click(within(rowOf(table, role)).getByRole('button', { name: `Change the model of ${role}` }));

  return (await screen.findByRole('textbox', { name: `Model for ${role}` })) as HTMLInputElement;
}

describe('Changing the model of a Harness Skills role', () => {
  it('offers Change and the thinking level on every row, never Map', async () => {
    const table = await openRoles();
    const rows = within(table).getAllByRole('row').slice(1);

    expect(rows.length).toBeGreaterThan(1);

    for (const row of rows) {
      // inherit takes the session's model, so it has no level to set.
      const inherits = within(row).queryByText('inherit') !== null;

      expect(within(row).getAllByRole('button')).toHaveLength(inherits ? 1 : 2);
      expect(within(row).getByRole('button', { name: /^Change the model of / })).toHaveTextContent('Change');
      expect(within(row).queryByRole('button', { name: /^Thinking level of / }) !== null).toBe(!inherits);
      expect(within(row).queryByRole('button', { name: /^Map / })).toBeNull();
    }
  });

  it('starts from the model the row has, saves the typed one and shows it in the row', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const table = await openRoles();

    const field = await openChange(table, 'frontend');
    expect(field.value).toBe('sonnet');

    await fireEvent.input(field, { target: { value: 'opus:high' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    await waitFor(() => expect(within(rowOf(table, 'frontend')).getByText('opus')).toBeInTheDocument());
    expect(invoke).toHaveBeenCalledWith('skills_set_role', { harness: 'claude', scope: 'user', project: null, role: 'frontend', selector: 'opus:high' });
    expect(within(rowOf(table, 'frontend')).getByText('high')).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole('textbox', { name: 'Model for frontend' })).toBeNull());
  });

  it('offers the aliases and the models already in the file, and saves the one chosen', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const table = await openRoles();

    await openChange(table, 'research');

    for (const name of ['opus', 'sonnet', 'haiku', 'fable', 'inherit', 'claude-haiku-5-5']) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument();
    }

    await fireEvent.click(screen.getByRole('button', { name: 'haiku' }));

    // research was claude-haiku-5-5:high: the alias keeps the level, which Claude Code takes on every alias.
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('skills_set_role', { harness: 'claude', scope: 'user', project: null, role: 'research', selector: 'haiku:high' }));
    await waitFor(() => expect(within(rowOf(table, 'research')).getByText('haiku')).toBeInTheDocument());
  });

  it('drops a level the new model does not offer', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const table = await openRoles();

    // backend is claude-opus-5-5:max, and the preview harness offers off to high.
    await openChange(table, 'backend');
    await fireEvent.click(await screen.findByRole('button', { name: 'claude-sonnet-4-5' }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('skills_set_role', { harness: 'claude', scope: 'user', project: null, role: 'backend', selector: 'claude-sonnet-4-5' }));
  });

  it('offers the levels Claude Code takes on an alias, even when the harness cannot list its models', async () => {
    // No helper agent can start, so the harness's list of models never comes.
    const original = ipc.invoke.bind(ipc);
    vi.spyOn(ipc, 'invoke').mockImplementation(((command: string, args: unknown) =>
      command === 'agent_start' ? Promise.reject({ code: 'unavailable', message: 'No helper here' }) : original(command as never, args as never)) as typeof ipc.invoke);
    const table = await openRoles();

    // plan is opus:xhigh, an alias the harness's list never has.
    await fireEvent.click(within(rowOf(table, 'plan')).getByRole('button', { name: 'Thinking level of plan: xhigh' }));
    const menu = await screen.findByRole('menu', { name: 'Thinking level of plan' });

    await waitFor(() => expect(within(menu).getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual(['Default', 'low', 'medium', 'high', 'xhigh', 'max']));
  });

  it('does not ask the backend for a model the harness would not accept', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const table = await openRoles();

    const field = await openChange(table, 'backend');
    await fireEvent.input(field, { target: { value: 'gpt-5.5' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    expect(screen.getByText('Use a model or model:effort, such as opus or claude-opus-5-5:high.')).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith('skills_set_role', expect.anything());
  });

  it('says why the change failed and keeps the popover and the row', async () => {
    const table = await openRoles();
    const model = (): string => within(rowOf(table, 'implementation')).getAllByRole('cell')[0].textContent ?? '';
    const before = model();
    vi.spyOn(ipc, 'invoke').mockRejectedValue({ code: 'conflict', message: 'The model roles changed while it was being edited' });

    const field = await openChange(table, 'implementation');
    await fireEvent.input(field, { target: { value: 'haiku' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0]).toMatchObject({ title: 'Could not change implementation', description: 'The model roles changed while it was being edited', tone: 'danger' });
    expect(screen.getByRole('textbox', { name: 'Model for implementation' })).toBeInTheDocument();
    expect(model()).toBe(before);
  });
});

describe('The model rules of each harness', () => {
  const rules = (id: 'claude' | 'opencode' | 'codex') => selectorRules(id)!.isValid;

  it('takes what the Harness Skills parser of that harness takes', () => {
    for (const text of ['opus', 'inherit', 'fable:max', 'anthropic/sonnet:low', 'claude-opus-5-5:xhigh']) {
      expect(rules('claude')(text), text).toBe(true);
    }

    for (const text of ['', 'gpt-5.5', 'opus:', 'opus:none', 'opus:high:low', 'Opus']) {
      expect(rules('claude')(text), text).toBe(false);
    }

    for (const text of ['inherit', 'anthropic/claude-sonnet-4-5:high', 'ollama/llama3:8b', 'openai/gpt-5:none']) {
      expect(rules('opencode')(text), text).toBe(true);
    }

    for (const text of ['', 'opus', '/gpt-5', 'openai/', 'inherit:high', 'Anthropic/x', 'openai/gpt-5:']) {
      expect(rules('opencode')(text), text).toBe(false);
    }

    for (const text of ['inherit', 'inherit:high', 'gpt-5.5', 'gpt-5.5:none', 'gpt-5.5:xhigh']) {
      expect(rules('codex')(text), text).toBe(true);
    }

    for (const text of ['', 'gpt 5', 'gpt-5.5:', 'inherit:', ':high']) {
      expect(rules('codex')(text), text).toBe(false);
    }
  });

  it('has none for a harness without model role files', () => {
    expect(selectorRules('pi')).toBeNull();
  });
});

describe('The thinking level of a role row', () => {
  const omp = selectorRules('omp')!;
  const claude = selectorRules('claude')!;
  const codex = selectorRules('codex')!;
  const listed: ModelOption[] = [{ provider: 'anthropic', id: 'claude-opus-5-5', name: 'Opus', reasoning: true, contextWindow: null, maxTokens: null, input: ['text'] }];

  it('splits off only a level the harness knows', () => {
    expect(splitSelector(omp, 'openai-codex/gpt-6-luna:xhigh')).toEqual({ model: 'openai-codex/gpt-6-luna', level: 'xhigh' });
    expect(splitSelector(omp, 'openai/gpt-5:max')).toEqual({ model: 'openai/gpt-5', level: 'max' });
    expect(splitSelector(omp, 'ollama/qwen3-coder:30b')).toEqual({ model: 'ollama/qwen3-coder:30b', level: null });
    expect(splitSelector(claude, 'opus:max')).toEqual({ model: 'opus', level: 'max' });
    expect(splitSelector(codex, 'gpt-5.5:max')).toEqual({ model: 'gpt-5.5:max', level: null });
  });

  it('joins a level, never onto a model that takes none', () => {
    expect(joinSelector(omp, 'openai/gpt-5', 'high')).toBe('openai/gpt-5:high');
    expect(joinSelector(omp, 'openai/gpt-5', null)).toBe('openai/gpt-5');
    expect(joinSelector(claude, 'inherit', 'high')).toBe('inherit');
  });

  it('asks about the model the row names, or the listed one its bare ID belongs to', () => {
    expect(modelToAsk(omp, 'openai-codex/gpt-6-luna', null)).toEqual({ provider: 'openai-codex', id: 'gpt-6-luna' });
    expect(modelToAsk(claude, 'claude-opus-5-5', listed)).toEqual({ provider: 'anthropic', id: 'claude-opus-5-5' });
    expect(modelToAsk(claude, 'anthropic/claude-opus-5-5', listed)).toEqual({ provider: 'anthropic', id: 'claude-opus-5-5' });
    expect(modelToAsk(claude, 'opus', listed)).toBeNull();
    expect(modelToAsk(omp, 'no-provider', null)).toBeNull();
    expect(modelToAsk(claude, 'inherit', listed)).toBeNull();
  });
});
