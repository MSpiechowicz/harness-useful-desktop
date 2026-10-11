import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import { harness, type HarnessStore } from '$lib/stores/harness.svelte';
import { prefs, type PrefsStore } from '$lib/stores/prefs.svelte';

import { intro } from '../intro/intro-state.svelte';

export type OnboardingStep = 'welcome' | 'harnesses' | 'project' | 'approval' | 'done';

/**
 * The first-run flow. It appears as the intro hands over, so the lockup is followed by the setup
 * and never by a flash of the shell, once preferences are loaded. It stays until the user finishes
 * it; "Run onboarding again" brings it back (and lets them close it).
 */
export class OnboardingController {
  step = $state<OnboardingStep>('welcome');
  /** The harness the user picked explicitly; null until they choose. */
  choice = $state<HarnessId | null>(null);
  /** Re-run from Settings or a call to action, after onboarding was completed once. */
  rerun = $state(false);
  /** +1 moving forward, -1 moving back; drives the step transition. */
  direction = $state<1 | -1>(1);

  constructor(
    private readonly preferences: Pick<PrefsStore, 'value' | 'update'>,
    private readonly harnesses: Pick<HarnessStore, 'summaries' | 'installed' | 'get'>,
    private readonly introFinished: () => boolean,
  ) {}

  get visible(): boolean {
    const value = this.preferences.value;
    if (!value || !this.introFinished()) {
      return false;
    }

    return this.rerun || !value.onboardingCompleted;
  }

  /** The daily driver: the explicit pick, else the saved default, else the first installed harness. */
  get driver(): HarnessId | null {
    const candidates = [this.choice, this.preferences.value?.defaultHarness ?? null];

    for (const id of candidates) {
      if (id && this.harnesses.get(id)?.detection.installed) {
        return id;
      }
    }

    return this.harnesses.installed[0]?.info.id ?? null;
  }

  /** OMP runs tools without asking by default; say so once, when OMP is the daily driver. */
  get needsApprovalNotice(): boolean {
    return this.driver === 'omp' && !this.preferences.value?.approvalNoticeSeen;
  }

  get steps(): OnboardingStep[] {
    return this.needsApprovalNotice ? ['welcome', 'harnesses', 'project', 'approval', 'done'] : ['welcome', 'harnesses', 'project', 'done'];
  }

  get index(): number {
    return this.steps.indexOf(this.step);
  }

  choose(id: HarnessId): void {
    this.choice = id;
  }

  next(): void {
    this.move(1);
  }

  back(): void {
    this.move(-1);
  }

  /** Opens the flow again, optionally at a later step. */
  restart(from: OnboardingStep = 'welcome'): void {
    this.choice = null;
    this.direction = 1;
    this.step = from;
    this.rerun = true;
  }

  /** Closes a re-run without changing anything. */
  dismiss(): void {
    this.rerun = false;
    this.step = 'welcome';
  }

  /** Stores the outcome. The flow stays open if saving failed (the prefs store shows the error). */
  async finish(): Promise<void> {
    await this.preferences.update({
      onboardingCompleted: true,
      defaultHarness: this.driver,
      approvalNoticeSeen: this.preferences.value?.approvalNoticeSeen || this.steps.includes('approval'),
    });

    if (this.preferences.value?.onboardingCompleted) {
      this.dismiss();
    }
  }

  private move(delta: 1 | -1): void {
    const target = this.steps[this.index + delta];
    if (target) {
      this.direction = delta;
      this.step = target;
    }
  }
}

export const onboarding = new OnboardingController(prefs, harness, () => intro.phase === 'handoff' || intro.phase === 'done');
