import type { MotionLevel } from '$lib/stores/motion.svelte';

/**
 * Timeline of the cold-start intro (milliseconds from start). The short launch of the shared Harness logo
 * takes 1.9 s (the leg falls off and becomes the cursor, two blinks); `launched()` starts the hand-over a
 * short hold after the logo reports it is done, and `HANDOFF_MS` is the fallback. Any key or click skips it all.
 *
 * The hand-over is two beats: the mark leaves (`INTRO_EXIT_MS`), then the next screen, the
 * shell or the setup, enters after `INTRO_LEAD_MS` while the veil behind the mark lifts.
 * The phase lasts until the slowest entrance has settled.
 */
export const HANDOFF_MS = 2400;
export const INTRO_EXIT_MS = 320;
export const INTRO_LEAD_MS = 240;
export const HANDOFF_DURATION_MS = 900;
export const TOTAL_MS = HANDOFF_MS + HANDOFF_DURATION_MS;
export const REDUCED_MS = 250;

export type IntroPhase = 'idle' | 'playing' | 'handoff' | 'done';

/**
 * Drives the intro sequence. The shell reads `phase` to hide, stagger in or show
 * its panels; the logo layer reads it to fade out.
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
      this.schedule(() => this.handOff(), HANDOFF_MS);
    }
  }

  /** The logo finished its launch: hand over to the shell without waiting for the fallback timer. */
  launched(): void {
    if (this.phase === 'playing') {
      this.clearTimers();
      this.handOff();
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

  private handOff(): void {
    this.phase = 'handoff';
    this.schedule(() => (this.phase = 'done'), HANDOFF_DURATION_MS);
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
