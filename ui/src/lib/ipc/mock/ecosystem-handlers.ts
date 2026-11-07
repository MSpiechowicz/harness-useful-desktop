import type { DashboardResult } from '../bindings/DashboardResult';
import type { DashboardTool } from '../bindings/DashboardTool';
import type { EcosystemStatus } from '../bindings/EcosystemStatus';
import type { JsonValue } from '../bindings/serde_json/JsonValue';
import { IpcFailure } from '../transport';
import { ecosystemFixtures as samples } from './ecosystem-fixtures';
import type { Handlers } from './types';

type EcosystemCommands = 'ecosystem_status' | 'dashboard_availability' | 'dashboard_query';

/**
 * Browser-preview switches: `?dashboard=missing|failed|slow`, `?ecosystem=failed` and `?limits=left|dense`.
 * `left` sends the plan limits the way the dashboard does when its own setting counts down (`leftPercent`), `dense` adds
 * windows until a plan has five. Without `limits` they are as recorded.
 */
export interface EcosystemSwitches {
  dashboard: 'ok' | 'missing' | 'failed' | 'slow';
  ecosystem: 'ok' | 'failed';
  limits?: 'left' | 'dense';
}

const SLOW_MS = 1800;

/** Costs are scaled from the 30-day fixture so each range shows plausible, distinct numbers. */
const BASE_RANGE = '30d';

/** The arguments each tool accepts, like the real validation (unknown fields are refused). */
const SHARED = ['range', 'from', 'to', 'provider', 'project', 'model', 'user', 'tag', 'kind'];
const ACCEPTED: Record<DashboardTool, string[]> = {
  usageSummary: SHARED,
  breakdown: [...SHARED, 'dimension', 'limit'],
  sessionCost: ['session_id', 'cwd'],
  branchCost: ['branch', 'project'],
  limits: [],
  budgetStatus: [],
  failures: [...SHARED, 'tool', 'limit'],
  tips: [...SHARED, 'limit'],
};

export function readSwitches(search: string = globalThis.location?.search ?? ''): EcosystemSwitches {
  const params = new URLSearchParams(search);
  const dashboard = params.get('dashboard');
  const ecosystem = params.get('ecosystem');
  const limits = params.get('limits');

  return {
    dashboard: dashboard === 'missing' || dashboard === 'failed' || dashboard === 'slow' ? dashboard : 'ok',
    ecosystem: ecosystem === 'failed' ? 'failed' : 'ok',
    limits: limits === 'left' || limits === 'dense' ? limits : undefined,
  };
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function invalid(message: string): IpcFailure {
  return IpcFailure.from({ code: 'invalidInput', message });
}

function argsOf(tool: DashboardTool, value: JsonValue): Record<string, JsonValue> {
  if (value === null) {
    return {};
  }
  if (typeof value !== 'object' || Array.isArray(value)) {
    throw invalid(`dashboard ${tool}: arguments must be an object`);
  }

  const unknown = Object.keys(value).find((key) => !ACCEPTED[tool].includes(key));
  if (unknown) {
    throw invalid(`dashboard ${tool}: unknown field \`${unknown}\``);
  }

  return value;
}

function fixtureOf(sample: DashboardResult): DashboardResult {
  return structuredClone(sample);
}

function dataOf(result: DashboardResult): Record<string, JsonValue> {
  return result.data as Record<string, JsonValue>;
}

function rangeName(args: Record<string, JsonValue>, fallback: string): string {
  return typeof args.range === 'string' ? args.range : fallback;
}

function usage(args: Record<string, JsonValue>): DashboardResult {
  return fixtureOf(samples.usage[rangeName(args, 'today')] ?? samples.usage.today);
}

/** The 30-day breakdown scaled to the requested range, by the ratio of the two usage summaries. */
function breakdown(args: Record<string, JsonValue>): DashboardResult {
  const dimension = args.dimension === 'provider' ? 'provider' : 'model';
  const result = fixtureOf(samples.breakdown[dimension]);
  const data = dataOf(result);
  const range = rangeName(args, BASE_RANGE);

  const base = dataOf(samples.usage[BASE_RANGE]);
  const wanted = dataOf(samples.usage[range] ?? samples.usage[BASE_RANGE]);
  const ratio = (wanted.costUsd as number) / (base.costUsd as number);

  const rows = (data.rows as Record<string, JsonValue>[]).slice(0, typeof args.limit === 'number' ? args.limit : undefined);
  for (const row of rows) {
    row.costUsd = Math.round((row.costUsd as number) * ratio * 10_000) / 10_000;
    row.tokens = Math.round((row.tokens as number) * ratio);
  }

  data.rows = rows;
  data.totalCostUsd = wanted.costUsd;
  data.totalTokens = (wanted.tokens as Record<string, number>).total;
  data.range = { ...(data.range as Record<string, JsonValue>), name: range };

  return result;
}

function sessionCost(args: Record<string, JsonValue>): DashboardResult {
  const id = typeof args.session_id === 'string' ? args.session_id : null;
  const known = dataOf(samples.sessionCost).sessionId as string;

  if (id !== null && !known.endsWith(id)) {
    const missing = fixtureOf(samples.sessionCostError);
    missing.text = `No session ${id} in the database yet. The dashboard scans every 30 seconds while it runs.`;

    return missing;
  }

  return fixtureOf(samples.sessionCost);
}

function branchCost(args: Record<string, JsonValue>): DashboardResult {
  const branch = typeof args.branch === 'string' ? args.branch.trim() : '';
  if (branch === '') {
    throw invalid('dashboard branch_cost: branch is required');
  }

  // Default branches carry no feature work in the sample data, so the chip stays hidden there.
  if (branch === 'main' || branch === 'master') {
    return { tool: 'branchCost', data: null, text: `No usage recorded on branch ${branch}.`, isError: true };
  }

  const result = fixtureOf(samples.branchCost);
  dataOf(result).branch = branch;

  return result;
}

/** When the limits sample was taken; its reset times are moved so they stay ahead of the real clock. */
const LIMITS_SAMPLED_AT = Date.parse('2026-10-09T12:30:00Z');

/** Windows added by `?limits=dense`, so one plan has five: a daily one close to its end and one with no length to measure a pace against. */
const EXTRA_WINDOWS: Record<string, JsonValue>[] = [
  { provider: 'claude', plan: 'max', window: '1d', usedPercent: 92, resetsAt: '2026-10-09T18:00:00.000Z', observedAt: '2026-10-09T12:27:00.000Z', ageMinutes: 3, status: 'fresh' },
  { provider: 'claude', plan: 'max', window: 'monthly', usedPercent: 33.4, resetsAt: '2026-10-31T00:00:00.000Z', observedAt: '2026-10-09T12:27:00.000Z', ageMinutes: 3, status: 'fresh' },
];

/** What the dashboard sends when its own setting counts down: the share left in place of the share used. */
function countDown(window: Record<string, JsonValue>): void {
  const used = window.usedPercent;

  if (typeof used === 'number') {
    delete window.usedPercent;
    window.leftPercent = Math.round((100 - used) * 10) / 10;
  }
}

function limits(variant: EcosystemSwitches['limits']): DashboardResult {
  const result = fixtureOf(samples.limits);
  const shift = Date.now() - LIMITS_SAMPLED_AT;
  const data = dataOf(result);

  if (variant === 'dense') {
    data.windows = [...(data.windows as Record<string, JsonValue>[]), ...structuredClone(EXTRA_WINDOWS)];
  }

  for (const window of data.windows as Record<string, JsonValue>[]) {
    if (variant === 'left') {
      countDown(window);
    }

    for (const key of ['resetsAt', 'observedAt']) {
      const value = window[key];

      if (typeof value === 'string') {
        window[key] = new Date(Date.parse(value) + shift).toISOString();
      }
    }
  }

  return result;
}

const READERS: Record<DashboardTool, (args: Record<string, JsonValue>, switches: EcosystemSwitches) => DashboardResult> = {
  usageSummary: usage,
  breakdown,
  sessionCost,
  branchCost,
  limits: (_, switches) => limits(switches.limits),
  budgetStatus: () => fixtureOf(samples.budgetStatus),
  failures: (args) => {
    const result = fixtureOf(samples.failures);
    const data = dataOf(result);
    data.range = { ...(data.range as Record<string, JsonValue>), name: rangeName(args, '7d') };

    return result;
  },
  tips: () => fixtureOf(samples.tips),
};

function status(switches: EcosystemSwitches): EcosystemStatus {
  const result = structuredClone(samples.status);

  if (switches.dashboard === 'missing') {
    const dashboard = result.items.find((item) => item.id === 'dashboard');
    if (dashboard) {
      Object.assign(dashboard, { installed: false, version: null, via: [] });
    }
  }

  return result;
}

/** The ecosystem and dashboard commands over the generated samples. */
export function ecosystemHandlers(switches: EcosystemSwitches = readSwitches()): Pick<Handlers, EcosystemCommands> {
  return {
    ecosystem_status: async () => {
      if (switches.ecosystem === 'failed') {
        throw IpcFailure.from({ code: 'internal', message: 'Could not read the installed plugins of OMP.' });
      }

      return status(switches);
    },

    dashboard_availability: async () => {
      if (switches.dashboard === 'slow') {
        await delay(SLOW_MS);
      }

      if (switches.dashboard === 'missing') {
        return { type: 'notInstalled' };
      }

      if (switches.dashboard === 'failed') {
        return { type: 'failed', message: 'harness-dashboard mcp did not answer within 15 seconds.' };
      }

      return { type: 'available' };
    },

    dashboard_query: async ({ query }) => {
      if (switches.dashboard === 'missing') {
        throw IpcFailure.from({ code: 'notFound', message: 'The Harness Dashboard is not installed.' });
      }

      if (switches.dashboard === 'slow') {
        await delay(SLOW_MS / 2);
      }

      return READERS[query.tool](argsOf(query.tool, query.args), switches);
    },
  };
}
