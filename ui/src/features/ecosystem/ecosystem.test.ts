import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { dashboard } from '$lib/stores/dashboard.svelte';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { freshBackend } from '../../../tests/ecosystem-backend';
import { actionCommands } from '../shell/palette/actions';
import EcosystemView from './EcosystemView.svelte';

beforeAll(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off' });
});

afterEach(() => vi.restoreAllMocks());

async function show(switches: Parameters<typeof freshBackend>[0] = {}) {
  const backend = freshBackend(switches);
  render(EcosystemView);

  return backend;
}

describe('Ecosystem view', () => {
  it('explains the ecosystem and lists the three companions with their state', async () => {
    await show();

    expect(await screen.findByRole('heading', { level: 2, name: 'Tools that make agents measurable' })).toBeInTheDocument();

    const dashboardCard = await screen.findByRole('article', { name: /Harness Dashboard/ });
    expect(within(dashboardCard).getByText('Installed')).toBeInTheDocument();
    expect(within(dashboardCard).getByText('v1.22.1')).toBeInTheDocument();

    // Where it was found is secondary detail, one click away.
    await fireEvent.click(within(dashboardCard).getByRole('button', { name: 'Details' }));
    expect(within(dashboardCard).getByText('binary ~/.local/bin/harness-dashboard')).toBeInTheDocument();

    const sidebar = screen.getByRole('article', { name: /Harness Sidebar/ });
    expect(within(sidebar).getByText('Not installed')).toBeInTheDocument();
    // The GitHub release lookup lands just after the local reading.
    expect(await screen.findByText('2 of 3 installed · 1 update available', { selector: 'p[role="status"]' })).toBeInTheDocument();
  });

  it('flags a companion whose GitHub release is newer and opens its update steps', async () => {
    await show();

    const skills = await screen.findByRole('article', { name: /Harness Skills/ });
    expect(await within(skills).findByText('Update available')).toBeInTheDocument();
    expect(within(skills).getByText('v0.2.40')).toBeInTheDocument();
    expect(within(skills).getByRole('button', { name: /How to update/ })).toHaveAttribute('aria-expanded', 'true');
  });

  it('shows copyable install steps for a missing companion and never runs them', async () => {
    const { calls } = await show();
    const sidebar = await screen.findByRole('article', { name: /Harness Sidebar/ });

    const steps = within(sidebar).getByRole('list', { name: 'Install steps' });
    expect(within(steps).getAllByRole('listitem')).toHaveLength(3);
    expect(within(steps).getByText('omp plugin marketplace add MSpiechowicz/harness-useful-sidebar')).toBeInTheDocument();

    const clipboard = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: clipboard }, configurable: true });
    await fireEvent.click(within(steps).getAllByRole('button', { name: /^Copy:/ })[1]);

    expect(clipboard).toHaveBeenCalledWith('omp plugin install harness-useful-sidebar@harness-useful-sidebar --scope user');
    expect(calls('extension_action')).toHaveLength(0);
  });

  it('opens the repository only through the allow-listed external opener', async () => {
    const { calls } = await show();
    const card = await screen.findByRole('article', { name: /Harness Skills/ });

    await fireEvent.click(within(card).getByRole('button', { name: /^GitHub/ }));

    await waitFor(() => expect(calls('open_external').map((call) => call.url)).toEqual(['https://github.com/MSpiechowicz/harness-useful-skills']));
  });

  it('fills the usage panel and follows the range switch', async () => {
    await show();

    expect(await screen.findByRole('article', { name: 'Cost, the last 7 days' })).toHaveTextContent('$38.41');
    expect(screen.getByRole('article', { name: 'Tokens, the last 7 days' })).toHaveTextContent('34.2M');
    expect(screen.getByRole('article', { name: 'Cost, the last 7 days' })).toHaveTextContent('+24%');

    await fireEvent.click(screen.getByRole('radio', { name: '30 days' }));

    expect(await screen.findByRole('article', { name: 'Cost, the last 30 days' })).toHaveTextContent('$142.91');
    expect(dashboard.range).toBe('30d');
  });

  it('charts the spend by model and by harness with exact values in text', async () => {
    await show();
    // The ring is one picture whose name carries every figure; the legend beside it is the readable table.
    const models = await screen.findByRole('img', { name: /^Cost by model, the last 7 days: \$\d/ });
    expect(models.getAttribute('aria-label')).toMatch(/claude-opus-5-5 \$23\.69 \(62%\)/);
    const modelRows = screen.getByRole('list', { name: 'Cost by model, the last 7 days, values' });
    expect(within(modelRows).getByText('claude-opus-5-5')).toBeInTheDocument();
    expect(within(modelRows).getByText('$23.69')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('radio', { name: 'By harness' }));

    const harnesses = await screen.findByRole('list', { name: 'Cost by harness, the last 7 days, values' });
    expect(within(harnesses).getAllByRole('listitem').map((row) => row.querySelector('[title]')?.textContent)).toEqual(['omp', 'claude', 'pi', 'codex']);
  });

  it('shows plan limits and budgets as meters with words, not only colors', async () => {
    await show();

    const nearCap = await screen.findByRole('meter', { name: 'Project · demo budget' });
    expect(nearCap).toHaveAttribute('aria-valuenow', '85');
    expect(nearCap).toHaveAttribute('aria-valuetext', '85%, high');
    expect(screen.getAllByText(/projected \$210\.90, over the cap/)).toHaveLength(1);

    const fiveHour = screen.getByRole('meter', { name: /^Claude Max 5-hour limit: 57% left/ });
    expect(fiveHour).toHaveAttribute('aria-valuenow', '57');
    expect(fiveHour).toHaveAttribute('aria-valuetext', '57% left');
    expect(screen.getByText('Weekly limit · Fable')).toBeInTheDocument();
    expect(screen.getByRole('meter', { name: /^Claude Max Weekly limit · Fable: 88% left/ })).toHaveAttribute('aria-valuenow', '88');
    // One provider per page: the other waits behind its tab, with a name that says where it sits.
    expect(screen.queryByRole('meter', { name: /^Codex/ })).toBeNull();
    const claudeTab = screen.getByRole('tab', { name: 'Provider 1 of 2: Claude' });
    expect(claudeTab).toHaveAttribute('aria-selected', 'true');

    await fireEvent.keyDown(claudeTab, { key: 'ArrowRight' });

    expect(screen.getByRole('tab', { name: 'Provider 2 of 2: Codex' })).toHaveAttribute('aria-selected', 'true');
    // A window that reset after it was recorded reads whole again, without a pace.
    const reset = screen.getByRole('meter', { name: 'Codex Pro 5-hour limit: 100% left, has reset since last seen' });
    expect(reset).toHaveAttribute('aria-valuenow', '100');
    expect(screen.queryByRole('meter', { name: /^Claude/ })).toBeNull();
  });

  it('lists recent failures and tips', async () => {
    await show();

    expect(await screen.findByText(/mismatched types: expected/)).toBeInTheDocument();
    expect(await screen.findByText('64 small prompts on a premium model')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Show all 4' }));

    expect(screen.getByText('2 prompts with 150+ tool calls')).toBeInTheDocument();
  });

  it('offers the install steps when the dashboard is not installed', async () => {
    await show({ dashboard: 'missing' });

    const heading = await screen.findByRole('heading', { name: 'See what your agents cost' });
    const card = heading.closest('section') as HTMLElement;

    expect(await within(card).findByText(/curl -fsSL https:\/\/raw\.githubusercontent\.com/)).toBeInTheDocument();
    expect(screen.queryByRole('radio', { name: '7 days' })).toBeNull();
  });

  it('explains a dashboard that failed and retries on request', async () => {
    const { calls } = await show({ dashboard: 'failed' });

    expect(await screen.findByText('The dashboard did not answer')).toBeInTheDocument();
    expect(screen.getByText(/did not answer within 15 seconds/)).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Try again' }));

    await waitFor(() => expect(calls('dashboard_availability')).toHaveLength(2));
  });

  it('shows an error with a retry when the companions cannot be read', async () => {
    await show({ ecosystem: 'failed' });

    expect(await screen.findByText('Could not look for companions', { selector: 'h2' })).toBeInTheDocument();
    expect(ecosystem.state).toBe('error');
    expect(screen.getByRole('button', { name: 'Try again', description: '' })).toBeInTheDocument();
  });
});

describe('palette', () => {
  it('refreshes the dashboard and shows the ecosystem', async () => {
    const { calls } = freshBackend();
    const command = actionCommands({ openShortcuts: () => {} }).find((entry) => entry.label === 'Ecosystem: refresh dashboard');

    command?.run();

    expect(nav.current).toBe('ecosystem');
    await waitFor(() => expect(calls('dashboard_query').length).toBeGreaterThan(0));
    expect(calls('ecosystem_status')).toHaveLength(1);

    nav.go('chat');
  });
});
