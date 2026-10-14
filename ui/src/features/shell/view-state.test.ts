import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { sessionList } from '$lib/stores/sessions.svelte';
import { viewState } from '$lib/stores/view-state.svelte';
import { configView, extensionsView } from '$lib/stores/views.svelte';

import { samplePrefs } from '../../../tests/helpers';
import ConfigViewer from '../config/ConfigViewer.svelte';
import ExtensionsViewer from '../extensions/ExtensionsViewer.svelte';
import SessionsView from '../sessions/SessionsView.svelte';
import { configCommands, extensionCommands, sessionCommands } from './palette/content';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp' });
  await harness.load();
  await project.pick();
});

describe('views remember where you were', () => {
  it('Config reopens on the file you had selected', async () => {
    const first = render(ConfigViewer);
    const files = await screen.findByRole('navigation', { name: 'Configuration files' });
    await fireEvent.click(within(files).getByText('MCP servers').closest('button')!);
    expect(within(files).getByText('MCP servers').closest('button')).toHaveAttribute('aria-current', 'true');
    first.unmount();

    render(ConfigViewer);

    const again = await screen.findByRole('navigation', { name: 'Configuration files' });
    expect(within(again).getByText('MCP servers').closest('button')).toHaveAttribute('aria-current', 'true');
  });

  it('Extensions reopens on the item you had open', async () => {
    const first = render(ExtensionsViewer);
    const skills = within(await screen.findByRole('region', { name: 'Skills' })).getAllByRole('button').slice(1);
    expect(skills.length).toBeGreaterThan(1);
    const chosen = skills[1].textContent;
    await fireEvent.click(skills[1]);
    first.unmount();

    render(ExtensionsViewer);

    const current = within(await screen.findByRole('region', { name: 'Skills' })).getAllByRole('button').find((button) => button.getAttribute('aria-current') === 'true');
    expect(current?.textContent).toBe(chosen);
  });

  it('Sessions reopens on the session you had selected', async () => {
    const first = render(SessionsView);
    await waitFor(() => expect(sessionList.data?.length ?? 0).toBeGreaterThan(1));
    const second = sessionList.data![1];
    await fireEvent.click(await screen.findByRole('button', { name: new RegExp(second.title ?? second.firstPrompt ?? '', 'i') }));
    expect(viewState.sessionId).toBe(second.ref.id);
    first.unmount();

    render(SessionsView);

    await waitFor(() => expect(document.querySelector(`[data-session="${second.ref.id}"][aria-current="true"]`)).not.toBeNull());
  });
});

describe('palette content', () => {
  it('jumps to a configuration key with that file open and the key filtered', async () => {
    configView.sync('omp', project.active?.path ?? null);
    await waitFor(() => expect(configView.data).not.toBeNull());

    const key = configCommands().find((command) => command.label === 'tools.approvalMode');
    expect(key?.detail).toContain('Settings');
    key?.run();

    expect(nav.current).toBe('config');
    render(ConfigViewer);
    expect(await screen.findByRole('searchbox', { name: 'Search keys and values' })).toHaveValue('approvalMode');
  });

  it('jumps to an extension on its own tab', async () => {
    extensionsView.sync('omp', project.active?.path ?? null);
    await waitFor(() => expect(extensionsView.data).not.toBeNull());

    const plugin = extensionCommands().find((command) => command.label === 'demo-plugin');
    plugin?.run();

    expect(nav.current).toBe('extensions');
    expect(viewState.extensionsTab).toBe('plugins');
    render(ExtensionsViewer);
    expect(await screen.findByRole('heading', { level: 2, name: 'demo-plugin' })).toBeInTheDocument();
  });

  it('offers sessions to open, with a secondary action that resumes them in chat', async () => {
    sessionList.sync('omp', project.active?.path ?? null);
    await waitFor(() => expect(sessionList.data?.length ?? 0).toBeGreaterThan(0));

    const [first] = sessionCommands();

    expect(first.group).toBe('Sessions');
    expect(first.secondary?.label).toBe('Resume in chat');
  });
});
