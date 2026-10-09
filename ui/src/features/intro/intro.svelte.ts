import type { MotionLevel } from '$lib/stores/motion.svelte';

/**
 * Timeline of the cold-start intro (milliseconds from start).
 * Everything the user waits for is over by `TOTAL_MS` (<= 1.6 s).
 */
export const HANDOFF_MS = 900;
export const TOTAL_MS = 1600;
export const REDUCED_MS = 250;

export type IntroPhase = 'idle' | 'playing' | 'handoff' | 'done';

/**
 * Drives the intro sequence. The shell reads `phase` to hide, stagger in or show
 * its panels; the aurora and logo layers read it to dim and fade out.
 */
export class IntroController {
  phase = $state<IntroPhase>('idle');

  private timers: ReturnType<typeof setTimeout>[] = [];

  /** Plays the intro for the given effective motion level. */
  play(level: MotionLevel): void {
    this.clearTimers();

    if (level === 'off') {
      this.phase = 'done';
    } else if (level === 'reduced') {
      this.phase = 'handoff';
      this.schedule(() => (this.phase = 'done'), REDUCED_MS);
    } else {
      this.phase = 'playing';
      this.schedule(() => (this.phase = 'handoff'), HANDOFF_MS);
      this.schedule(() => (this.phase = 'done'), TOTAL_MS);
    }
  }

  /** Jumps straight to the finished state (any key or click while playing). */
  skip(): void {
    this.clearTimers();
    this.phase = 'done';
  }

  /** The intro is neither skipped nor disabled: input should skip it. */
  get active(): boolean {
    return this.phase === 'playing' || this.phase === 'handoff';
  }

  private schedule(action: () => void, ms: number): void {
    this.timers.push(setTimeout(action, ms));
  }

  private clearTimers(): void {
    this.timers.forEach(clearTimeout);
    this.timers = [];
  }
}

export const intro = new IntroController();
