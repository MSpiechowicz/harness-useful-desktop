import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import { harness, type HarnessStore } from '$lib/stores/harness.svelte';
import { prefs, type PrefsStore } from '$lib/stores/prefs.svelte';

import { intro } from '../intro/intro-state.svelte';
import { type ApprovalNotice, rememberNotices, seenNotices } from './approval-notices';
import { machineFacts } from './machine-facts.svelte';

export type OnboardingStep = 'welcome' | 'meet' | 'harnesses' | 'project' | 'approval' | 'family' | 'setup' | 'done';

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
  /** A step is changing the machine (the setup step's run): the flow cannot be left until it ends. */
  busy = $state(false);

  /** The notices that were on screen in this run, so one is only marked as seen when shown (a re-run may start past it). */
  private shownNotices = new Set<ApprovalNotice>();
  /** Bumped when a notice is remembered, so what depends on `approvalNotice` follows. */
  private remembered = $state(0);

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

  /**
   * The daily driver: the explicit pick, else the saved default, else the first installed harness that can chat
   * (chat is what a daily driver is for), else the first installed one. Any installed harness may be picked, one
   * without chat included: the other views follow it.
   */
  get driver(): HarnessId | null {
    const candidates = [this.choice, this.preferences.value?.defaultHarness ?? null];

    for (const id of candidates) {
      if (id && this.harnesses.get(id)?.detection.installed) {
        return id;
      }
    }

    const installed = this.harnesses.installed;

    return (installed.find((summary) => summary.capabilities.chat) ?? installed[0])?.info.id ?? null;
  }

  /**
   * Which harness's "tools run without asking" notice to show, once: the one of the daily driver, since that is the
   * harness the user works with. OMP and OpenCode both run tools without asking by default, so either driver gets
   * its notice. Claude Code asks by default, so its driver gets one only when its own settings choose a mode that
   * runs tools unattended. Anything else gets none. Each notice is shown once: seeing one does not hide the other.
   */
  get approvalNotice(): ApprovalNotice | null {
    const driver = this.driver;

    if (driver !== 'omp' && driver !== 'opencode' && driver !== 'claude') {
      return null;
    }

    if (driver === 'claude' && !this.harnesses.get('claude')?.nativeApprovalMode?.unattended) {
      return null;
    }

    // Reading the counter makes a notice remembered by `finish` take effect here.
    void this.remembered;

    return seenNotices(this.preferences.value?.approvalNoticeSeen ?? false).has(driver) ? null : driver;
  }

  get needsApprovalNotice(): boolean {
    return this.approvalNotice !== null;
  }

  get steps(): OnboardingStep[] {
    return this.needsApprovalNotice
      ? ['welcome', 'meet', 'harnesses', 'project', 'approval', 'family', 'setup', 'done']
      : ['welcome', 'meet', 'harnesses', 'project', 'family', 'setup', 'done'];
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
    this.shownNotices = new Set();
    machineFacts.reset();
    this.rerun = true;
    this.noteNotice(from);
  }

  /** Closes a re-run without changing anything; ignored while a step is busy. */
  dismiss(): void {
    if (this.busy) {
      return;
    }

    this.rerun = false;
    this.step = 'welcome';
  }

  /** Stores the outcome. The flow stays open if saving failed (the prefs store shows the error). */
  async finish(): Promise<void> {
    // The saved flag is the OMP notice's, from before there were two.
    await this.preferences.update({
      onboardingCompleted: true,
      defaultHarness: this.driver,
      approvalNoticeSeen: this.preferences.value?.approvalNoticeSeen || this.shownNotices.has('omp'),
    });
    rememberNotices(this.shownNotices);
    this.remembered += 1;

    if (this.preferences.value?.onboardingCompleted) {
      this.dismiss();
    }
  }

  private move(delta: 1 | -1): void {
    if (this.busy) {
      return;
    }

    const target = this.steps[this.index + delta];
    if (target) {
      this.direction = delta;
      this.step = target;
      this.noteNotice(target);
    }
  }

  /** Records which notice the approval step shows now, when that is the step being entered. */
  private noteNotice(step: OnboardingStep): void {
    const notice = this.approvalNotice;

    if (step === 'approval' && notice) {
      this.shownNotices.add(notice);
    }
  }
}

export const onboarding = new OnboardingController(prefs, harness, () => intro.phase === 'handoff' || intro.phase === 'done');
