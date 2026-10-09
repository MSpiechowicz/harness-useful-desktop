import {
  readBranchCost,
  readBreakdown,
  readBudgets,
  readFailures,
  readLimits,
  readSessionCost,
  readTips,
  readUsageSummary,
  type Breakdown,
  type Budget,
  type CostSummary,
  type Failures,
  type LimitWindow,
  type Tip,
  type UsageSummary,
} from '$lib/dashboard/models';
import type { DashboardAvailability } from '$lib/ipc/bindings/DashboardAvailability';
import type { DashboardResult } from '$lib/ipc/bindings/DashboardResult';
import type { DashboardTool } from '$lib/ipc/bindings/DashboardTool';
import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

/** The ranges the panel offers; the dashboard knows more (month, all, custom). */
export const PANEL_RANGES = ['today', '7d', '30d'] as const;
export type PanelRange = (typeof PANEL_RANGES)[number];

export type PanelDimension = 'model' | 'provider';

/** Tips are drawn from a month of usage whatever range the tiles show. */
const TIPS_RANGE: PanelRange = '30d';

/** How long a successful availability answer is trusted, and a negative one (so a install is noticed soon). */
const AVAILABLE_TTL_MS = 5 * 60_000;
const UNAVAILABLE_TTL_MS = 30_000;

/** The dashboard rescans its logs every 30 seconds, so a fresher cost than this is not available. */
const COST_TTL_MS = 30_000;
const PANEL_TTL_MS = 30_000;

export type SlotStatus = 'idle' | 'loading' | 'ready' | 'error';

/**
 * One query of the panel. While it reloads (another range, a refresh) it keeps the previous data,
 * so the panel dims instead of flashing skeletons. A slow answer for a request that was superseded
 * never overwrites a newer one.
 */
export class QuerySlot<T> {
  data = $state.raw<T | null>(null);
  status = $state<SlotStatus>('idle');
  error = $state<string | null>(null);

  private token = 0;

  /** True while reloading data that is already on screen. */
  get stale(): boolean {
    return this.status === 'loading' && this.data !== null;
  }

  reset(): void {
    this.token += 1;
    this.data = null;
    this.error = null;
    this.status = 'idle';
  }

  async run(task: () => Promise<T>): Promise<void> {
    this.token += 1;
    const mine = this.token;
    this.status = 'loading';

    try {
      const data = await task();

      if (mine === this.token) {
        this.data = data;
        this.error = null;
        this.status = 'ready';
      }
    } catch (error) {
      if (mine === this.token) {
        this.data = null;
        this.error = IpcFailure.from(error).message;
        this.status = 'error';
      }
    }
  }
}

export interface CostEntry {
  /** `unavailable` hides the chip: no dashboard, no data for this session or branch, or a failure. */
  status: 'loading' | 'ready' | 'unavailable';
  summary: CostSummary | null;
  at: number;
}

export type CostTool = Extract<DashboardTool, 'sessionCost' | 'branchCost'>;

function costKey(tool: CostTool, args: Record<string, string>): string {
  return `${tool}\u0000${JSON.stringify(args)}`;
}

/** The Harness Dashboard as the app sees it: whether it answers, the Ecosystem panel and the cost chips. */
export class DashboardStore {
  /** `null` until the first check has finished. */
  availability = $state.raw<DashboardAvailability | null>(null);

  range = $state<PanelRange>('7d');
  dimension = $state<PanelDimension>('model');

  usage = new QuerySlot<UsageSummary>();
  byModel = new QuerySlot<Breakdown>();
  byProvider = new QuerySlot<Breakdown>();
  failures = new QuerySlot<Failures>();
  tips = new QuerySlot<Tip[]>();
  limits = new QuerySlot<LimitWindow[]>();
  budgets = new QuerySlot<Budget[]>();

  private costs = $state.raw<Record<string, CostEntry>>({});
  private checking: Promise<DashboardAvailability> | null = null;
  private checkedAt = 0;
  private panelLoadedAt = 0;
  private pending = new Map<string, Promise<void>>();

  constructor(
    private readonly transport: Transport,
    private readonly now: () => number = Date.now,
  ) {}

  /** Whether the dashboard answers. Positive answers are trusted for minutes, negative ones for seconds. */
  async ensureAvailability(force = false): Promise<DashboardAvailability> {
    const current = this.availability;
    const ttl = current?.type === 'available' ? AVAILABLE_TTL_MS : UNAVAILABLE_TTL_MS;

    if (!force && current && this.now() - this.checkedAt < ttl) {
      return current;
    }

    this.checking ??= this.check().finally(() => (this.checking = null));

    return this.checking;
  }

  /** Loads the panel unless it was loaded moments ago; `force` always reloads. */
  async openPanel(force = false): Promise<void> {
    const availability = await this.ensureAvailability(force);

    if (availability.type !== 'available') {
      return;
    }

    if (!force && this.panelLoadedAt > 0 && this.now() - this.panelLoadedAt < PANEL_TTL_MS) {
      return;
    }

    this.panelLoadedAt = this.now();
    await Promise.all([this.loadRange(), this.limits.run(() => this.read('limits', {}, readLimits)), this.budgets.run(() => this.read('budgetStatus', {}, readBudgets))]);
  }

  async setRange(range: PanelRange): Promise<void> {
    if (range === this.range) {
      return;
    }

    this.range = range;
    await this.loadRange();
  }

  /** The cost entry of a chip, or undefined before it was requested. */
  cost(tool: CostTool, args: Record<string, string>): CostEntry | undefined {
    return this.costs[costKey(tool, args)];
  }

  /** Fetches the cost of one session or branch unless a fresh answer is cached; never throws. */
  requestCost(tool: CostTool, args: Record<string, string>, force = false): Promise<void> {
    const key = costKey(tool, args);
    const cached = this.costs[key];

    if (!force && cached && cached.status !== 'loading' && this.now() - cached.at < COST_TTL_MS) {
      return Promise.resolve();
    }

    const running = this.pending.get(key);
    if (running) {
      return running;
    }

    const request = this.fetchCost(key, tool, args).finally(() => this.pending.delete(key));
    this.pending.set(key, request);

    return request;
  }

  /** Forgets everything, as at startup (and between tests). */
  reset(): void {
    this.availability = null;
    this.checkedAt = 0;
    this.panelLoadedAt = 0;
    this.costs = {};
    this.checking = null;
    this.pending.clear();
    this.range = '7d';
    this.dimension = 'model';

    for (const slot of [this.usage, this.byModel, this.byProvider, this.failures, this.tips, this.limits, this.budgets]) {
      slot.reset();
    }
  }

  private async check(): Promise<DashboardAvailability> {
    let next: DashboardAvailability;

    try {
      next = await this.transport.invoke('dashboard_availability', undefined);
    } catch (error) {
      next = { type: 'failed', message: IpcFailure.from(error).message };
    }

    this.availability = next;
    this.checkedAt = this.now();

    return next;
  }

  private loadRange(): Promise<unknown> {
    const range = this.range;

    return Promise.all([
      this.usage.run(() => this.read('usageSummary', { range }, readUsageSummary)),
      this.byModel.run(() => this.read('breakdown', { dimension: 'model', range, limit: 8 }, readBreakdown)),
      this.byProvider.run(() => this.read('breakdown', { dimension: 'provider', range, limit: 8 }, readBreakdown)),
      this.failures.run(() => this.read('failures', { range, limit: 6 }, readFailures)),
      this.tips.run(() => this.read('tips', { range: TIPS_RANGE, limit: 6 }, readTips)),
    ]);
  }

  /** One tool call; a tool-reported error becomes a readable failure, unexpected data too. */
  private async read<T>(tool: DashboardTool, args: Record<string, JsonValue>, parse: (result: DashboardResult) => T): Promise<T> {
    const result = await this.transport.invoke('dashboard_query', { query: { tool, args } });

    if (result.isError) {
      throw new Error(result.text ?? 'The dashboard reported an error.');
    }

    return parse(result);
  }

  private async fetchCost(key: string, tool: CostTool, args: Record<string, string>): Promise<void> {
    const previous = this.costs[key];
    this.costs = { ...this.costs, [key]: { status: 'loading', summary: previous?.summary ?? null, at: this.now() } };

    const availability = await this.ensureAvailability();
    let entry: CostEntry = { status: 'unavailable', summary: null, at: this.now() };

    if (availability.type === 'available') {
      try {
        const parse = tool === 'sessionCost' ? readSessionCost : readBranchCost;
        entry = { status: 'ready', summary: await this.read(tool, args, parse), at: this.now() };
      } catch {
        // The chip is a convenience: no data (a session the dashboard has not seen yet) just hides it.
      }
    }

    this.costs = { ...this.costs, [key]: entry };
  }
}

export const dashboard = new DashboardStore(ipc);
