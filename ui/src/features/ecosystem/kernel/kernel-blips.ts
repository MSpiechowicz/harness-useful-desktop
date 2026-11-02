import type { ModuleState } from './kernel-members';

/** One blip crosses a trace in this many seconds, and rests on the others' time between. */
const TRAVEL_S = 1.1;
/** A loaded module sends a blip every 3.1 to 4.7 s, each with its own offset, so the traces never fire together. */
const PERIOD_S = 3.1;
const PERIOD_STEP_S = 0.8;
const PERIOD_VARIANTS = 3;
const PHASE_STEP_S = 0.9;
/** A module with an update waits for one now and then, coming from the chip. */
const UPDATE_PERIOD_S = 2.6;
const UPDATE_PHASE_S = 0.4;
/** The share of the trip a blip spends fading in and out. */
const FADE = 0.12;

export interface Blip {
  /** How far along the route, 0 (module) to 1 (chip). */
  u: number;
  opacity: number;
}

/** Where the blip of a module is at `seconds`, or `null` while the trace rests. */
export function blipAt(index: number, state: ModuleState, seconds: number): Blip | null {
  if (state === 'missing') {
    return null;
  }

  const update = state === 'update';
  const period = update ? UPDATE_PERIOD_S : PERIOD_S + (index % PERIOD_VARIANTS) * PERIOD_STEP_S;
  const phase = update ? UPDATE_PHASE_S : index * PHASE_STEP_S;
  const local = (((seconds + phase) % period) + period) % period;

  if (local >= TRAVEL_S) {
    return null;
  }

  const progress = local / TRAVEL_S;

  return { u: update ? 1 - progress : progress, opacity: Math.min(1, progress / FADE, (1 - progress) / FADE) };
}
