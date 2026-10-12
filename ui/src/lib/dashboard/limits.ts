import { t, tn } from '$lib/i18n/i18n.svelte';
import { locale } from '$lib/utils/format';

import { formatSpan } from './format';
import type { LimitWindow } from './models';

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

/** Earlier than this into a window, or with this little used, a projection would be noise. */
const MIN_ELAPSED_SHARE = 0.05;
const MIN_USED_SHARE = 0.02;

/** A time further out than this is written with its weekday. */
const SHORT_CLOCK_MS = 20 * HOUR_MS;

const PROVIDER_NAMES: Record<string, string> = {
  claude: 'Claude',
  codex: 'Codex',
  copilot: 'GitHub Copilot',
  'google-gemini-cli': 'Gemini',
  xai: 'xAI',
  deepseek: 'DeepSeek',
  'opencode-go': 'OpenCode Go',
};

/** Limit providers that are a subscription of a harness wear that harness's color. */
const PROVIDER_HARNESS: Record<string, string> = {
  'opencode-go': 'opencode',
};

const PLAN_NAMES: Record<string, string> = {
  prolite: 'Pro Lite',
  max: 'Max',
  pro: 'Pro',
  plus: 'Plus',
  free: 'Free',
  team: 'Team',
  business: 'Business',
  enterprise: 'Enterprise',
  edu: 'Edu',
};

function titleCase(value: string): string {
  return value.replace(/[_-]+/g, ' ').replace(/\b\w/g, (letter) => letter.toUpperCase());
}

export function providerName(provider: string): string {
  return PROVIDER_NAMES[provider.toLowerCase()] ?? titleCase(provider);
}

export function planName(plan: string): string {
  return PLAN_NAMES[plan.toLowerCase()] ?? titleCase(plan);
}

/** The plan's name beside its provider, or null when it only repeats the provider ("OpenCode Go" on OpenCode Go). */
export function planLabel(provider: string, plan: string | null): string | null {
  if (plan === null) {
    return null;
  }

  const name = planName(plan);
  return name.toLowerCase() === providerName(provider).toLowerCase() ? null : name;
}

/** The provider with its plan when the plan adds something: "Claude Max", "OpenCode Go". */
export function planSubject(provider: string, plan: string | null): string {
  const label = planLabel(provider, plan);
  return label ? `${providerName(provider)} ${label}` : providerName(provider);
}

/** The harness whose color a limit provider wears. */
export function providerHarness(provider: string): string {
  return PROVIDER_HARNESS[provider.toLowerCase()] ?? provider;
}

/** How long a window lasts, from its name ("5h", "7d"); null for a provider's own window name. */
export function windowLengthMs(window: string): number | null {
  const match = /^(\d+)([hd])$/.exec(window.trim());
  if (!match) {
    return null;
  }

  const count = Number(match[1]);

  return count > 0 ? count * (match[2] === 'd' ? DAY_MS : HOUR_MS) : null;
}

/** "5-hour limit", "Weekly limit", "Weekly limit · Fable": the window's name plus its scope. */
export function windowName(window: string, scope: string | null): string {
  const length = windowLengthMs(window);
  const base = length === null ? window : lengthName(length);

  return scope ? `${base} · ${scope}` : base;
}

function lengthName(length: number): string {
  if (length === DAY_MS) {
    return t('ecosystem.limits.daily');
  }
  if (length === 7 * DAY_MS) {
    return t('ecosystem.limits.weekly');
  }
  if (length % DAY_MS === 0) {
    return tn('ecosystem.limits.days', length / DAY_MS);
  }

  return tn('ecosystem.limits.hours', length / HOUR_MS);
}

/** The share of a window that is left, 0..1. A window that has reset since the reading is whole again. */
export function leftShare(usedPercent: number | null): number {
  if (usedPercent === null) {
    return 1;
  }

  return Math.max(0, Math.min(1, 1 - usedPercent / 100));
}

export type LeftTone = 'ok' | 'low' | 'critical';

/** Plenty left keeps the provider's color; little left turns to a warning, and almost none to danger. */
export function leftTone(left: number): LeftTone {
  if (left >= 0.25) {
    return 'ok';
  }

  return left >= 0.1 ? 'low' : 'critical';
}

export interface Pace {
  /** The share of the window that is still ahead, 0..1. */
  timeLeft: number;
  /** When the limit runs out if use keeps its pace, in ms; null when it is too early to tell. */
  outAt: number | null;
}

/**
 * Where a window stands in time and where its use is heading: the use so far, kept up, against the time left.
 * `null` when the window length or reset time is unknown, the reset has passed, or the window has reset since the reading.
 */
export function paceOf(window: LimitWindow, now: number): Pace | null {
  const length = windowLengthMs(window.window);
  const resetsAt = window.resetsAt === null ? Number.NaN : Date.parse(window.resetsAt);

  if (length === null || window.usedPercent === null || Number.isNaN(resetsAt) || resetsAt <= now) {
    return null;
  }

  const remaining = resetsAt - now;
  const timeLeft = Math.min(1, remaining / length);
  const elapsed = length - remaining;
  const used = Math.min(1, window.usedPercent / 100);

  if (elapsed < length * MIN_ELAPSED_SHARE || used < MIN_USED_SHARE) {
    return { timeLeft, outAt: null };
  }

  return { timeLeft, outAt: now + ((1 - used) * elapsed) / used };
}

/** Whether the limit runs out before the window resets. */
export function runsOutBeforeReset(pace: Pace, resetsAt: string | null): boolean {
  if (pace.outAt === null || resetsAt === null) {
    return false;
  }

  return pace.outAt < Date.parse(resetsAt);
}

/** "12:06" within a day of `now`, "Mon 12:06" further out. */
export function formatClock(time: number, now: number): string {
  const options: Intl.DateTimeFormatOptions =
    time - now > SHORT_CLOCK_MS
      ? { weekday: 'short', hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }
      : { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' };

  return new Intl.DateTimeFormat(locale(), options).format(time);
}

/** "12m ago", "6h ago", "3d ago". */
export function formatAge(minutes: number): string {
  if (minutes < 60) {
    return t('ecosystem.time.ago', { time: formatSpan(Math.max(1, Math.round(minutes)), 'minute') });
  }
  if (minutes < 24 * 60) {
    return t('ecosystem.time.ago', { time: formatSpan(Math.round(minutes / 60), 'hour') });
  }

  return t('ecosystem.time.ago', { time: formatSpan(Math.round(minutes / (24 * 60)), 'day') });
}

export interface LimitGroup {
  key: string;
  provider: string;
  plan: string | null;
  /** Minutes since the newest reading, set only when none in the group is fresh. */
  staleMinutes: number | null;
  windows: LimitWindow[];
}

/** Windows grouped under their provider and plan, in the order they first appear. */
export function groupLimits(windows: LimitWindow[]): LimitGroup[] {
  const groups = new Map<string, LimitGroup>();

  for (const window of windows) {
    const key = `${window.provider}\u0000${window.plan ?? ''}`;
    let group = groups.get(key);

    if (!group) {
      group = { key, provider: window.provider, plan: window.plan, staleMinutes: null, windows: [] };
      groups.set(key, group);
    }

    group.windows.push(window);
  }

  for (const group of groups.values()) {
    group.staleMinutes = staleMinutes(group.windows);
  }

  return [...groups.values()];
}

/** How many plans the card pages through: the ones in use. */
export const SHOWN_PLANS = 2;

/**
 * The plans in use, one per provider, at most `count` of them, in the dashboard's order.
 * The dashboard refreshes every signed-in plan whether it is used or not, so a fresh reading alone
 * means little: a plan counts as in use when a window still shows some of it spent (a reading from
 * before a reset carries no amount). With none in use, the most recently read plans stand in so
 * the card is never empty.
 */
export function recentGroups(groups: LimitGroup[], count = SHOWN_PLANS): LimitGroup[] {
  const inUse = groups.filter((group) => group.windows.some((window) => (window.usedPercent ?? 0) > 0));
  const pool = inUse.length > 0 ? inUse : [...groups].sort((a, b) => recency(a) - recency(b));
  const shown = new Map<string, LimitGroup>();

  for (const group of pool) {
    if (!shown.has(group.provider)) {
      shown.set(group.provider, group);
    }
  }

  return [...shown.values()].slice(0, count);
}

/** Minutes since the group's newest reading; a fresh reading counts as now, an undated one as oldest. */
function recency(group: LimitGroup): number {
  if (group.windows.some((window) => window.status === 'fresh')) {
    return 0;
  }

  return group.staleMinutes ?? Number.POSITIVE_INFINITY;
}

function staleMinutes(windows: LimitWindow[]): number | null {
  const ages = windows.map((window) => window.ageMinutes).filter((age): age is number => age !== null);
  if (ages.length === 0 || windows.some((window) => window.status === 'fresh')) {
    return null;
  }

  return Math.min(...ages);
}
