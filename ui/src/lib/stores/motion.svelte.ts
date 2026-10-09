import { MediaQuery } from '$lib/design/theme.svelte';
import type { MotionPref } from '$lib/ipc/bindings/MotionPref';

import { prefs } from './prefs.svelte';

export type MotionLevel = MotionPref;

/** The more restrictive of the in-app setting and the OS `prefers-reduced-motion`. */
export function effectiveMotion(preference: MotionPref, osReduces: boolean): MotionLevel {
  if (preference === 'full' && osReduces) {
    return 'reduced';
  }

  return preference;
}

export class MotionStore {
  private readonly osReduces: MediaQuery;

  constructor(
    private readonly preference: () => MotionPref,
    osReduces: MediaQuery = new MediaQuery('(prefers-reduced-motion: reduce)'),
    private readonly root: HTMLElement = document.documentElement,
  ) {
    this.osReduces = osReduces;
  }

  get level(): MotionLevel {
    return effectiveMotion(this.preference(), this.osReduces.matches);
  }

  /** Mirrors the effective level onto `<html data-motion>`; returns a disposer. */
  start(): () => void {
    return $effect.root(() => {
      $effect(() => {
        this.root.dataset.motion = this.level;
      });
    });
  }
}

export const motion = new MotionStore(() => prefs.value?.motion ?? 'full');
