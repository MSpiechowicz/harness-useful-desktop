import { i18n, t } from '$lib/i18n/i18n.svelte';
import type { LimitGaugePref } from '$lib/ipc/bindings/LimitGaugePref';
import type { LimitBarsPref } from '$lib/ipc/bindings/LimitBarsPref';
import { locale } from '$lib/utils/format';

import { timeUntil } from './format';
import { formatClock, MIN_ELAPSED_SHARE, MIN_USED_SHARE, shortWindowName, windowLengthMs, windowName } from './limits';
import type { LimitWindow } from './models';

/**
 * What a plan-limit chart draws, ported from the Harness Dashboard's Live card (web/src/components/PlanLimits.svelte) so
 * the two apps show a limit the same way: the share used or left, where the use is heading by the reset, and a chip.
 */
export type LimitMode = LimitBarsPref;

/** What a chart draws, as fractions of its scale from the 0% end: the reading, where the solid part stops, and the pale forecast stretch. */
export interface Dial {
  value: number;
  solid: number;
  ahead: { from: number; to: number } | null;
}

/** Where the use is heading: `projected` is the share used by the reset (above 1 it runs out before), `outAt` when it runs out, in ms. */
export interface LimitPace {
  projected: number;
  outAt: number;
}

/** The chip under a chart: one style for every state, only the color changes. */
export interface LimitChip {
  kind: 'ok' | 'out' | 'idle';
  text: string;
}

/** Everything one chart and its text need for one window. */
export interface LimitView {
  window: LimitWindow;
  name: string;
  shortName: string;
  pace: LimitPace | null;
  chip: LimitChip | null;
  dial: Dial;
  /** The number the chart reads out: used or left, as a whole percent. */
  percent: number;
  /** The number in words: "43% used". */
  reading: string;
  reset: string;
  /** The exact reset time, for a tooltip on the countdown. */
  resetTitle: string | undefined;
  /** What a screen reader hears for the chart: what it is, how much, how it is heading, and when it resets. */
  label: string;
}

/** The smallest stretch of forecast worth drawing, as a share of the scale. */
const MIN_ARC = 0.005;

/** Under this long to the reset the reading is about to go: the chip says so instead of judging the pace. */
const RESETTING_MS = 30_000;

/** A window with no share used has reset since the reading: it is whole again. */
function hasReset(window: LimitWindow): boolean {
  return window.usedPercent === null;
}

/** The share of the window used, 0..1. */
export function usedShare(window: LimitWindow): number {
  if (window.usedPercent === null) {
    return 0;
  }

  return Math.max(0, Math.min(1, window.usedPercent / 100));
}

function resetsAtOf(window: LimitWindow): number | null {
  if (window.resetsAt === null) {
    return null;
  }

  const time = Date.parse(window.resetsAt);

  return Number.isNaN(time) ? null : time;
}

/**
 * Where the use is heading: the use so far, kept up, over the whole window. Above 1 it runs out before the reset.
 * `null` when the window has no length to measure against (a provider's own name), the reset is unknown or past, or it is
 * too early in the window, or with next to nothing used, for the projection to be more than noise.
 */
export function limitPace(window: LimitWindow, now: number): LimitPace | null {
  const length = windowLengthMs(window.window);
  const resetsAt = resetsAtOf(window);

  if (hasReset(window) || length === null || resetsAt === null || resetsAt <= now) {
    return null;
  }

  const used = usedShare(window);
  const elapsed = length - (resetsAt - now);

  if (elapsed < length * MIN_ELAPSED_SHARE || used < MIN_USED_SHARE) {
    return null;
  }

  return { projected: used / (elapsed / length), outAt: now + ((1 - used) * elapsed) / used };
}

/** Used some, but the window has only just begun: the pace says nothing yet. */
export function tooEarly(window: LimitWindow, now: number): boolean {
  const length = windowLengthMs(window.window);
  const resetsAt = resetsAtOf(window);

  if (hasReset(window) || length === null || resetsAt === null || resetsAt <= now) {
    return false;
  }

  return length - (resetsAt - now) < length * MIN_ELAPSED_SHARE;
}

/** The chip under a chart. None when there is nothing to say: used, but no window to measure the pace against. */
export function chipOf(window: LimitWindow, pace: LimitPace | null, now: number): LimitChip | null {
  const resetsAt = resetsAtOf(window);
  const used = usedShare(window);

  // At the reset, before the next reading: the use shown is about to go, so the pace no longer counts. Under 30s reads as "0m".
  if (!hasReset(window) && resetsAt !== null && resetsAt - now < RESETTING_MS) {
    return { kind: 'idle', text: t('ecosystem.limits.chip.resetting') };
  }
  if (used < MIN_USED_SHARE) {
    return { kind: 'idle', text: t('ecosystem.limits.chip.idle') };
  }
  if (used >= 1) {
    return { kind: 'out', text: t('ecosystem.limits.chip.reached') };
  }
  if (pace) {
    return pace.projected > 1
      ? { kind: 'out', text: t('ecosystem.limits.chip.out', { time: formatClock(pace.outAt, now) }) }
      : { kind: 'ok', text: t('ecosystem.limits.chip.ok') };
  }
  if (tooEarly(window, now)) {
    return { kind: 'idle', text: t('ecosystem.limits.chip.early') };
  }

  return null;
}

/**
 * What the chart draws. The solid part ends at the needle. With the forecast, used: a pale stretch runs on from the needle
 * to where the use will have got by the reset. Left: the solid part stops where the reset will leave what is left, and the
 * pale stretch fills in up to the needle. Both end at the red end when it runs out.
 */
export function dialOf(window: LimitWindow, pace: LimitPace | null, mode: LimitMode, forecast: boolean): Dial {
  const used = usedShare(window);
  const value = mode === 'used' ? used : 1 - used;

  if (!forecast || !pace) {
    return { value, solid: value, ahead: null };
  }

  if (mode === 'used') {
    const to = Math.min(1, pace.projected);

    return { value, solid: value, ahead: to - value > MIN_ARC ? { from: value, to } : null };
  }

  const to = Math.max(0, 1 - pace.projected);

  return value - to > MIN_ARC ? { value, solid: to, ahead: { from: to, to: value } } : { value, solid: value, ahead: null };
}

/** When the window resets, in words: it may already have, or never have said. */
export function resetText(window: LimitWindow, now: number): string {
  if (hasReset(window)) {
    return t('ecosystem.limits.resetSince');
  }
  if (window.resetsAt === null) {
    return t('ecosystem.limits.noReset');
  }

  const countdown = timeUntil(window.resetsAt, now);
  if (countdown) {
    return t('ecosystem.limits.resets', { time: countdown });
  }

  return t('ecosystem.limits.resetSince');
}

function resetTitleOf(window: LimitWindow, now: number): string | undefined {
  const resetsAt = resetsAtOf(window);

  return hasReset(window) || resetsAt === null || resetsAt <= now ? undefined : new Date(resetsAt).toLocaleString(locale());
}

/** The window as one chart and its text show it, with the pace, chip and screen-reader label worked out once. */
export function limitView(window: LimitWindow, subject: string, mode: LimitMode, forecast: boolean, now: number): LimitView {
  const pace = limitPace(window, now);
  const chip = chipOf(window, pace, now);
  const dial = dialOf(window, pace, mode, forecast);
  const name = windowName(window.window, window.scope);
  const percent = Math.round(dial.value * 100);
  const reset = resetText(window, now);
  const reading = t(mode === 'used' ? 'ecosystem.limits.used' : 'ecosystem.limits.left', { percent });
  const details = [reading, chip?.text, reset].filter(Boolean).join(', ');

  return {
    window,
    name,
    shortName: shortWindowName(window.window, window.scope),
    pace,
    chip,
    dial,
    percent,
    reading,
    reset,
    resetTitle: resetTitleOf(window, now),
    label: t('ecosystem.limits.label', { subject, name, details }),
  };
}

export const LIMIT_GAUGES: readonly LimitGaugePref[] = ['bars', 'tach', 'segments', 'vu', 'thermo', 'waffle', 'battery'];

/** The chart styles to pick from, sorted by their name in the language on screen. */
export function gaugeOptions(): { value: LimitGaugePref; label: string }[] {
  return LIMIT_GAUGES.map((value) => ({ value, label: t(`ecosystem.limits.gauge.${value}`) })).sort((a, b) => a.label.localeCompare(b.label, i18n.locale));
}
