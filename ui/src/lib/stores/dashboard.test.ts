import { describe, expect, it } from 'vitest';

import { ecosystemHandlers, type EcosystemSwitches } from '$lib/ipc/mock/ecosystem-handlers';
import { defaultScenario } from '$lib/ipc/mock/scenarios';
import { createMockTransport } from '$lib/ipc/mock/transport';
import type { Transport } from '$lib/ipc/transport';

import { DashboardStore } from './dashboard.svelte';
import { EcosystemStore } from './ecosystem.svelte';

/** The mock backend with the ecosystem switches of a browser preview, every call recorded. */
function backend(switches: Partial<EcosystemSwitches> = {}) {
  const scenario = defaultScenario();
  scenario.handlers = { ...scenario.handlers, ...ecosystemHandlers({ dashboard: 'ok', ecosystem: 'ok', ...switches }) };

  const mock = createMockTransport(scenario, 0);
  const queries: { tool: string; args: unknown }[] = [];
  let availabilityChecks = 0;

  const transport: Transport = {
    invoke(name, args) {
      if (name === 'dashboard_query') {
        queries.push((args as { query: { tool: string; args: unknown } }).query);
      }
      if (name === 'dashboard_availability') {
        availabilityChecks += 1;
      }

      return mock.invoke(name, args);
    },
  };

  return { transport, queries, checks: () => availabilityChecks };
}

describe('dashboard panel', () => {
  it('loads every section for the range and keeps the numbers of the range it shows', async () => {
    const { transport, queries } = backend();
    const store = new DashboardStore(transport);

    await store.openPanel();

    expect(store.availability).toEqual({ type: 'available' });
    expect(store.usage.data?.costUsd).toBeCloseTo(38.4127, 4);
    expect(store.byModel.data?.rows[0].key).toBe('claude-opus-5-5');
    expect(store.byProvider.data?.rows.map((row) => row.key)).toEqual(['omp', 'claude', 'pi', 'codex']);
    expect(store.limits.data).toHaveLength(5);
    expect(store.budgets.data?.map((budget) => budget.scope)).toEqual(['daily', 'monthly', 'project']);
    expect(queries.find((query) => query.tool === 'usageSummary')?.args).toEqual({ range: '7d' });
  });

  it('switching range asks again for the range sections only', async () => {
    const { transport, queries } = backend();
    const store = new DashboardStore(transport);
    await store.openPanel();
    queries.length = 0;

    await store.setRange('30d');

    expect(store.usage.data?.costUsd).toBeCloseTo(142.9071, 4);
    expect(store.byModel.data?.totalCostUsd).toBeCloseTo(142.9071, 4);
    expect(queries.map((query) => query.tool).sort()).toEqual(['breakdown', 'breakdown', 'failures', 'tips', 'usageSummary']);
    expect(queries.every((query) => (query.args as { range?: string }).range !== undefined)).toBe(true);
  });

  it('shows the old numbers, flagged stale, while a new range loads', async () => {
    const { transport } = backend();
    const store = new DashboardStore(transport);
    await store.openPanel();

    const loading = store.setRange('today');

    expect(store.usage.stale).toBe(true);
    expect(store.usage.data?.costUsd).toBeCloseTo(38.4127, 4);

    await loading;

    expect(store.usage.stale).toBe(false);
    expect(store.usage.data?.costUsd).toBeCloseTo(4.8312, 4);
  });

  it('does not touch the panel when the dashboard is not installed', async () => {
    const { transport, queries } = backend({ dashboard: 'missing' });
    const store = new DashboardStore(transport);

    await store.openPanel();

    expect(store.availability).toEqual({ type: 'notInstalled' });
    expect(queries).toHaveLength(0);
    expect(store.usage.status).toBe('idle');
  });

  it('reports a dashboard that cannot be used, and checks again on request', async () => {
    const { transport, checks } = backend({ dashboard: 'failed' });
    const store = new DashboardStore(transport);

    await store.openPanel();
    await store.openPanel();

    expect(store.availability).toMatchObject({ type: 'failed' });
    expect(checks()).toBe(1);

    await store.openPanel(true);

    expect(checks()).toBe(2);
  });

  it('turns an error of one tool into that section’s message only', async () => {
    const { transport } = backend();
    const failing: Transport = {
      invoke: (name, args) => {
        if (name === 'dashboard_query' && (args as { query: { tool: string } }).query.tool === 'tips') {
          return Promise.resolve({ tool: 'tips', data: null, text: 'No usage recorded yet.', isError: true }) as never;
        }

        return transport.invoke(name, args);
      },
    };
    const store = new DashboardStore(failing);

    await store.openPanel();

    expect(store.tips.status).toBe('error');
    expect(store.tips.error).toBe('No usage recorded yet.');
    expect(store.usage.status).toBe('ready');
  });
});

describe('cost chips', () => {
  const SESSION = { session_id: '0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57' };

  it('reads the cost of a session once while the answer is fresh', async () => {
    const { transport, queries } = backend();
    const store = new DashboardStore(transport);

    await Promise.all([store.requestCost('sessionCost', SESSION), store.requestCost('sessionCost', SESSION)]);
    await store.requestCost('sessionCost', SESSION);

    const entry = store.cost('sessionCost', SESSION);
    expect(entry?.status).toBe('ready');
    expect(entry?.summary?.costUsd).toBeCloseTo(2.341, 3);
    expect(queries.filter((query) => query.tool === 'sessionCost')).toHaveLength(1);
  });

  it('asks again when forced, or when the answer has aged', async () => {
    const { transport, queries } = backend();
    let now = 0;
    const store = new DashboardStore(transport, () => now);

    await store.requestCost('branchCost', { branch: 'feat/empty-input', project: '/p' });
    now = 10_000;
    await store.requestCost('branchCost', { branch: 'feat/empty-input', project: '/p' });
    expect(queries).toHaveLength(1);

    await store.requestCost('branchCost', { branch: 'feat/empty-input', project: '/p' }, true);
    now = 120_000;
    await store.requestCost('branchCost', { branch: 'feat/empty-input', project: '/p' });

    expect(queries).toHaveLength(3);
  });

  it('marks a session the dashboard does not know as unavailable instead of failing', async () => {
    const { transport } = backend();
    const store = new DashboardStore(transport);
    const unknown = { session_id: '0199d000-0000-7000-8000-000000000001' };

    await store.requestCost('sessionCost', unknown);

    expect(store.cost('sessionCost', unknown)).toMatchObject({ status: 'unavailable', summary: null });
  });

  it('never queries when the dashboard is not installed', async () => {
    const { transport, queries } = backend({ dashboard: 'missing' });
    const store = new DashboardStore(transport);

    await store.requestCost('sessionCost', SESSION);

    expect(store.cost('sessionCost', SESSION)?.status).toBe('unavailable');
    expect(queries).toHaveLength(0);
  });
});

describe('ecosystem status', () => {
  it('lists the four companions and reuses a fresh reading', async () => {
    let reads = 0;
    const { transport } = backend();
    const counting: Transport = {
      invoke: (name, args) => {
        reads += name === 'ecosystem_status' ? 1 : 0;

        return transport.invoke(name, args);
      },
    };
    const store = new EcosystemStore(counting);

    await store.load();
    await store.load();

    expect(store.status?.items.map((item) => item.id)).toEqual(['dashboard', 'memory', 'skills', 'sidebar']);
    expect(reads).toBe(1);

    await store.load(true);

    expect(reads).toBe(2);
  });

  it('keeps the failure for the view to show', async () => {
    const { transport } = backend({ ecosystem: 'failed' });
    const store = new EcosystemStore(transport);

    await store.load();

    expect(store.state).toBe('error');
    expect(store.error).toMatch(/plugins/);
    expect(store.status).toBeNull();
  });
});
