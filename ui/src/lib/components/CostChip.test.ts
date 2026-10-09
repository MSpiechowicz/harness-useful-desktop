import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { prefs } from '$lib/stores/prefs.svelte';

import { freshBackend } from '../../../tests/ecosystem-backend';
import { samplePrefs } from '../../../tests/helpers';
import CostChip from './CostChip.svelte';

beforeAll(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off' });
});

afterEach(() => vi.restoreAllMocks());

describe('CostChip', () => {
  it('shows the cost of the session and its details on request', async () => {
    freshBackend();
    render(CostChip, { props: { kind: 'session', args: { session_id: '0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57' } } });

    const chip = await screen.findByRole('button', { name: 'Session cost $2.34. Show details' });
    expect(chip).toHaveTextContent('$2.34');

    await fireEvent.click(chip);

    const details = screen.getByRole('dialog', { name: 'Session cost' });
    expect(within(details).getByText('Handle empty input in the record parser')).toBeInTheDocument();
    expect(within(details).getByRole('list', { name: 'Cost by model' })).toHaveTextContent('claude-opus-5-5');
  });

  it('shows the cost of the branch of the project', async () => {
    const { calls } = freshBackend();
    render(CostChip, { props: { kind: 'branch', args: { branch: 'feat/empty-input', project: '/home/demo/Projects/demo' } } });

    expect(await screen.findByRole('button', { name: /^Branch cost \$7\.88/ })).toBeInTheDocument();
    expect(calls('dashboard_query')[0].query).toEqual({ tool: 'branchCost', args: { branch: 'feat/empty-input', project: '/home/demo/Projects/demo' } });
  });

  it('stays out of the way when the dashboard has nothing for this session', async () => {
    const { calls } = freshBackend();
    render(CostChip, { props: { kind: 'session', args: { session_id: '0199d000-0000-7000-8000-000000000001' } } });

    await waitFor(() => expect(calls('dashboard_query')).toHaveLength(1));

    expect(screen.queryByRole('button', { name: /cost/i })).toBeNull();
  });

  it('stays out of the way, and asks nothing, when the dashboard is not installed', async () => {
    const { calls } = freshBackend({ dashboard: 'missing' });
    render(CostChip, { props: { kind: 'session', args: { session_id: '0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57' } } });

    await waitFor(() => expect(calls('dashboard_availability')).toHaveLength(1));

    expect(screen.queryByRole('button', { name: /cost/i })).toBeNull();
    expect(calls('dashboard_query')).toHaveLength(0);
  });

  it('asks again when told that new usage is likely', async () => {
    const { calls } = freshBackend();
    const view = render(CostChip, { props: { kind: 'session', args: { session_id: '0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57' }, refreshKey: false } });
    await screen.findByRole('button', { name: /Session cost/ });

    await view.rerender({ kind: 'session', args: { session_id: '0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57' }, refreshKey: true });

    await waitFor(() => expect(calls('dashboard_query')).toHaveLength(2));
  });

  it('asks nothing while there is no session or branch to ask about', async () => {
    const { calls } = freshBackend();
    render(CostChip, { props: { kind: 'session', args: null } });

    await Promise.resolve();

    expect(calls('dashboard_query')).toHaveLength(0);
    expect(screen.queryByRole('button')).toBeNull();
  });
});
