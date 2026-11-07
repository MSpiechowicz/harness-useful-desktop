import { t, tn } from '$lib/i18n/i18n.svelte';
import { locale } from '$lib/utils/format';
import { ownEntry } from '$lib/utils/own-entry';

import { formatSpan } from './format';
import type { LimitWindow } from './models';

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

/** Earlier than this into a window, or with this little used, a projection would be noise. */
export const MIN_ELAPSED_SHARE = 0.05;
export const MIN_USED_SHARE = 0.02;

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
  return ownEntry(PROVIDER_NAMES, provider.toLowerCase()) ?? titleCase(provider);
}

export function planName(plan: string): string {
  return ownEntry(PLAN_NAMES, plan.toLowerCase()) ?? titleCase(plan);
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
  return ownEntry(PROVIDER_HARNESS, provider.toLowerCase()) ?? provider;
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

/** The name for a narrow gauge: the scope alone when there is one ("Fable"), otherwise the length of the window ("Week"). */
export function shortWindowName(window: string, scope: string | null): string {
  if (scope) {
    return scope;
  }

  const length = windowLengthMs(window);
  if (length === null) {
    return window;
  }
  if (length === DAY_MS) {
    return t('ecosystem.limits.short.day');
  }
  if (length === 7 * DAY_MS) {
    return t('ecosystem.limits.short.week');
  }
  if (length % DAY_MS === 0) {
    return tn('ecosystem.limits.short.days', length / DAY_MS);
  }

  return tn('ecosystem.limits.short.hours', length / HOUR_MS);
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

/**
 * The plans in use, one per provider, in the dashboard's order.
 * The dashboard refreshes every signed-in plan whether it is used or not, so a fresh reading alone
 * means little: a plan counts as in use when a window still shows some of it spent (a reading from
 * before a reset carries no amount). With none in use, the most recently read plans stand in so
 * the card is never empty.
 */
export function recentGroups(groups: LimitGroup[]): LimitGroup[] {
  const inUse = groups.filter((group) => group.windows.some((window) => (window.usedPercent ?? 0) > 0));
  const pool = inUse.length > 0 ? inUse : [...groups].sort((a, b) => recency(a) - recency(b));
  const shown = new Map<string, LimitGroup>();

  for (const group of pool) {
    if (!shown.has(group.provider)) {
      shown.set(group.provider, group);
    }
  }

  return [...shown.values()];
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
