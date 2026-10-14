import type { DashboardResult } from '$lib/ipc/bindings/DashboardResult';
import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';

/**
 * Typed views of the dashboard's `structuredContent`. The wire data is JSON of a tool we do not
 * own, so every reader checks what it takes and fails with a readable message, never a crash.
 */
export class DashboardShapeError extends Error {
  constructor(tool: string) {
    super(`The dashboard answered ${tool} in a form this version of Harness Desktop does not understand.`);
    this.name = 'DashboardShapeError';
  }
}

type Json = { [key: string]: JsonValue };

function record(value: JsonValue | undefined): Json | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? value : null;
}

function list(value: JsonValue | undefined): Json[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.flatMap((item) => {
    const entry = record(item);

    return entry ? [entry] : [];
  });
}

function number(source: Json, key: string): number | null {
  const value = source[key];

  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function text(source: Json, key: string): string | null {
  const value = source[key];

  return typeof value === 'string' ? value : null;
}

function strings(value: JsonValue | undefined): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];
}

function rangeName(data: Json): string | null {
  const range = record(data.range);

  return range ? text(range, 'name') : null;
}

function root(result: DashboardResult): Json {
  const data = record(result.data);
  if (!data) {
    throw new DashboardShapeError(result.tool);
  }

  return data;
}

function required(result: DashboardResult, source: Json, key: string): number {
  const value = number(source, key);
  if (value === null) {
    throw new DashboardShapeError(result.tool);
  }

  return value;
}

export interface TokenBreakdown {
  total: number;
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  reasoning: number;
}

function tokenBreakdown(value: JsonValue | undefined): TokenBreakdown {
  const source = record(value) ?? {};

  return {
    total: number(source, 'total') ?? 0,
    input: number(source, 'input') ?? 0,
    output: number(source, 'output') ?? 0,
    cacheRead: number(source, 'cacheRead') ?? 0,
    cacheWrite: number(source, 'cacheWrite') ?? 0,
    reasoning: number(source, 'reasoning') ?? 0,
  };
}

export interface UsageSummary {
  /** The range these numbers cover (`today`, `7d`, `30d`). */
  range: string | null;
  costUsd: number;
  tokens: TokenBreakdown;
  modelCalls: number;
  sessions: number;
  prompts: number;
  cacheHitRate: number | null;
  costPerPromptUsd: number | null;
  /** Relative change of the cost against the period before (`-0.126` is 12.6% less). */
  costChange: number | null;
  previousCostUsd: number | null;
  note: string | null;
}

export function readUsageSummary(result: DashboardResult): UsageSummary {
  const data = root(result);
  const previous = record(data.previousPeriod);

  return {
    range: rangeName(data),
    costUsd: required(result, data, 'costUsd'),
    tokens: tokenBreakdown(data.tokens),
    modelCalls: number(data, 'modelCalls') ?? 0,
    sessions: number(data, 'sessions') ?? 0,
    prompts: number(data, 'prompts') ?? 0,
    cacheHitRate: number(data, 'cacheHitRate'),
    costPerPromptUsd: number(data, 'costPerPromptUsd'),
    costChange: previous ? number(previous, 'costChange') : null,
    previousCostUsd: previous ? number(previous, 'costUsd') : null,
    note: text(data, 'note'),
  };
}

export interface BreakdownRow {
  key: string;
  costUsd: number;
  share: number;
  tokens: number;
  sessions: number;
  /** Share of the row's input tokens read from the cache, 0..1; null when the dashboard did not say. */
  cacheHitRate: number | null;
  estimatedPrice: boolean;
}

export interface Breakdown {
  range: string | null;
  dimension: string;
  totalCostUsd: number;
  totalTokens: number;
  rows: BreakdownRow[];
}

export function readBreakdown(result: DashboardResult): Breakdown {
  const data = root(result);

  return {
    range: rangeName(data),
    dimension: text(data, 'dimension') ?? '',
    totalCostUsd: required(result, data, 'totalCostUsd'),
    totalTokens: number(data, 'totalTokens') ?? 0,
    rows: list(data.rows).map((row) => ({
      key: text(row, 'key') ?? 'unknown',
      costUsd: number(row, 'costUsd') ?? 0,
      share: number(row, 'share') ?? 0,
      tokens: number(row, 'tokens') ?? 0,
      sessions: number(row, 'sessions') ?? 0,
      cacheHitRate: number(row, 'cacheHitRate'),
      estimatedPrice: row.estimatedPrice === true,
    })),
  };
}

export interface LimitWindow {
  provider: string;
  plan: string | null;
  window: string;
  /** A sub-limit within the window, e.g. one model family's own cap; null for the plan-wide limit. */
  scope: string | null;
  /** 0..100, or null when the last reading no longer applies. */
  usedPercent: number | null;
  resetsAt: string | null;
  /** Minutes since the dashboard recorded this reading; null when it did not say. */
  ageMinutes: number | null;
  status: string;
}

export function readLimits(result: DashboardResult): LimitWindow[] {
  const data = root(result);

  return list(data.windows).map((window) => ({
    provider: text(window, 'provider') ?? 'unknown',
    plan: text(window, 'plan'),
    window: text(window, 'window') ?? '',
    scope: text(window, 'scope'),
    usedPercent: number(window, 'usedPercent'),
    resetsAt: text(window, 'resetsAt'),
    ageMinutes: number(window, 'ageMinutes'),
    status: text(window, 'status') ?? 'unknown',
  }));
}

export interface Budget {
  scope: string;
  period: string;
  project: string | null;
  capUsd: number;
  spentUsd: number;
  usedPercent: number;
  projectedUsd: number | null;
  status: string;
}

export function readBudgets(result: DashboardResult): Budget[] {
  const data = root(result);

  return list(data.budgets).map((budget) => ({
    scope: text(budget, 'scope') ?? 'budget',
    period: text(budget, 'period') ?? '',
    project: text(budget, 'project'),
    capUsd: number(budget, 'capUsd') ?? 0,
    spentUsd: number(budget, 'spentUsd') ?? 0,
    usedPercent: number(budget, 'usedPercent') ?? 0,
    projectedUsd: number(budget, 'projectedUsd'),
    status: text(budget, 'status') ?? 'ok',
  }));
}

export interface FailureCause {
  cause: string;
  count: number;
  share: number;
  tools: string[];
}

export interface RecentFailure {
  time: string;
  tool: string;
  cause: string;
  error: string;
  provider: string | null;
}

export interface Failures {
  range: string | null;
  calls: number;
  failed: number;
  declined: number;
  errorRate: number;
  causes: FailureCause[];
  recent: RecentFailure[];
}

export function readFailures(result: DashboardResult): Failures {
  const data = root(result);
  const totals = record(data.totals) ?? {};

  return {
    range: rangeName(data),
    calls: number(totals, 'calls') ?? 0,
    failed: number(totals, 'failed') ?? 0,
    declined: number(totals, 'declined') ?? 0,
    errorRate: number(totals, 'errorRate') ?? 0,
    causes: list(data.causes).map((cause) => ({
      cause: text(cause, 'cause') ?? 'unknown',
      count: number(cause, 'count') ?? 0,
      share: number(cause, 'share') ?? 0,
      tools: strings(cause.tools),
    })),
    recent: list(data.recent).map((failure) => ({
      time: text(failure, 'time') ?? '',
      tool: text(failure, 'tool') ?? 'unknown',
      cause: text(failure, 'cause') ?? 'unknown',
      error: text(failure, 'error') ?? '',
      provider: text(failure, 'provider'),
    })),
  };
}

export interface Tip {
  id: string;
  severity: string;
  category: string;
  title: string;
  detail: string;
  impactUsdPerMonth: number | null;
}

export function readTips(result: DashboardResult): Tip[] {
  const data = root(result);

  return list(data.tips).map((tip, index) => ({
    id: text(tip, 'id') ?? String(index),
    severity: text(tip, 'severity') ?? 'info',
    category: text(tip, 'category') ?? '',
    title: text(tip, 'title') ?? '',
    detail: text(tip, 'detail') ?? '',
    impactUsdPerMonth: number(tip, 'impactUsdPerMonth'),
  }));
}

/** What a cost chip and its popover need from `session_cost` and `branch_cost`. */
export interface CostSummary {
  /** `session` or `branch`. */
  kind: 'session' | 'branch';
  title: string | null;
  costUsd: number;
  tokens: number;
  modelCalls: number;
  prompts: number;
  /** Cost of the subagents, already included in `costUsd`. */
  subagentCostUsd: number | null;
  models: { model: string; costUsd: number }[];
  commits: number | null;
  linesAdded: number | null;
  linesRemoved: number | null;
}

function models(value: JsonValue | undefined): { model: string; costUsd: number }[] {
  return list(value).map((entry) => ({ model: text(entry, 'model') ?? 'unknown', costUsd: number(entry, 'costUsd') ?? 0 }));
}

export function readSessionCost(result: DashboardResult): CostSummary {
  const data = root(result);
  const subagents = record(data.subagents);
  const lines = record(data.linesChanged);

  return {
    kind: 'session',
    title: text(data, 'title'),
    costUsd: required(result, data, 'costUsd'),
    tokens: tokenBreakdown(data.tokens).total,
    modelCalls: number(data, 'modelCalls') ?? 0,
    prompts: number(data, 'prompts') ?? 0,
    subagentCostUsd: subagents ? number(subagents, 'costUsd') : null,
    models: models(data.models),
    commits: number(data, 'commits'),
    linesAdded: lines ? number(lines, 'added') : null,
    linesRemoved: lines ? number(lines, 'removed') : null,
  };
}

export function readBranchCost(result: DashboardResult): CostSummary {
  const data = root(result);
  const lines = record(data.linesChanged);

  return {
    kind: 'branch',
    title: text(data, 'branch'),
    costUsd: required(result, data, 'costUsd'),
    tokens: number(data, 'tokens') ?? 0,
    modelCalls: number(data, 'modelCalls') ?? 0,
    prompts: number(data, 'prompts') ?? 0,
    subagentCostUsd: null,
    models: models(data.models),
    commits: number(data, 'commits'),
    linesAdded: lines ? number(lines, 'added') : null,
    linesRemoved: lines ? number(lines, 'removed') : null,
  };
}
