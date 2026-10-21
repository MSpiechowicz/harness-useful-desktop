import { t } from '$lib/i18n/i18n.svelte';
import type { UpdateInfo } from '$lib/ipc/bindings/UpdateInfo';
import type { UpdateProgress } from '$lib/ipc/bindings/UpdateProgress';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { agentSessions } from './agentSession.svelte';
import { app } from './app.svelte';
import { prefs, type PrefsStore } from './prefs.svelte';
import { toasts, type ToastsStore } from './toasts.svelte';

/** Why a check or an install did not work, in the terms the UI explains. */
export type UpdateFailureKind = 'network' | 'verification' | 'manifest' | 'installer' | 'busy' | 'stale' | 'unknown';

/**
 * idle -> checking -> upToDate | available | notConfigured | failed(check)
 * available -> downloading -> installing -> readyToRelaunch, or failed(install) with the update kept for a retry.
 */
export type UpdateState =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'upToDate'; checkedAt: Date }
  | { kind: 'notConfigured' }
  | { kind: 'available'; info: UpdateInfo }
  | { kind: 'downloading'; info: UpdateInfo; progress: UpdateProgress | null }
  | { kind: 'installing'; info: UpdateInfo }
  | { kind: 'readyToRelaunch'; info: UpdateInfo }
  | { kind: 'failed'; during: 'check' | 'install'; reason: UpdateFailureKind; info: UpdateInfo | null };

type Failed = Extract<UpdateState, { kind: 'failed' }>;

/** What the environment tells the store; injected so the policy is testable without stores of the app. */
export interface UpdateEnvironment {
  /** The installer shuts the app down itself (Windows). */
  installerClosesApp: () => boolean;
  /** Closing the app now would cut off running agents. */
  agentsRunning: () => boolean;
  /** How many chats are in the middle of a turn, which a restart would cut off. */
  workingChats: () => number;
}

/** The failure a rejected check or install means; `unsupported` is the build without an update key. */
export function classify(error: unknown): { notConfigured: true } | { notConfigured: false; reason: UpdateFailureKind } {
  switch (IpcFailure.from(error).code) {
    case 'unsupported':
      return { notConfigured: true };

    case 'forbidden':
      return { notConfigured: false, reason: 'verification' };

    case 'protocol':
      return { notConfigured: false, reason: 'manifest' };

    case 'timeout':
    case 'io':
      return { notConfigured: false, reason: 'network' };

    case 'process':
      return { notConfigured: false, reason: 'installer' };

    case 'invalidInput':
      return { notConfigured: false, reason: 'busy' };

    case 'notFound':
      return { notConfigured: false, reason: 'stale' };

    default:
      return { notConfigured: false, reason: 'unknown' };
  }
}

/** Failures worth trying the same install again; the others need a fresh check (or a human). */
export function retryable(reason: UpdateFailureKind): boolean {
  return reason === 'network' || reason === 'installer';
}

/** How often an open app looks for a newer release, and the shortest gap between two looks (focus events come in bursts). */
const RECHECK_MS = 30 * 60_000;
const MIN_GAP_MS = 60_000;

/** Checking for, downloading and installing app updates, following the update policy of the preferences. */
export class UpdatesStore {
  state = $state<UpdateState>({ kind: 'idle' });

  /** Notices the user sent away with "Later"; a new version or a new stage brings the notice back. */
  private dismissed = $state<string[]>([]);
  private started = false;
  private rechecking = false;
  private lastLook = 0;

  /** Whether the restart waits for the user's word, and how many working chats it would stop. */
  confirmingRestart = $state(false);
  chatsAtRisk = $state(0);

  constructor(
    private readonly transport: Transport,
    private readonly preferences: Pick<PrefsStore, 'value'>,
    private readonly notifications: ToastsStore,
    private readonly environment: UpdateEnvironment,
  ) {}

  /** The update being worked on, in any stage. */
  get info(): UpdateInfo | null {
    const current = this.state;

    return 'info' in current ? current.info : null;
  }

  /**
   * Download progress as a fraction: 1 once the download is done and the install runs (so the bar
   * does not drop back to a sweep while its fill is still catching up), `null` when the size is
   * unknown or nothing is downloading.
   */
  get fraction(): number | null {
    const current = this.state;
    if (current.kind === 'installing') {
      return 1;
    }

    if (current.kind !== 'downloading' || !current.progress?.total) {
      return null;
    }

    return Math.min(1, current.progress.downloaded / current.progress.total);
  }

  /** The installer shuts the app down itself, so there is no separate restart (Windows). */
  get closesApp(): boolean {
    return this.environment.installerClosesApp();
  }

  get busy(): boolean {
    return ['checking', 'downloading', 'installing', 'readyToRelaunch'].includes(this.state.kind);
  }

  /** Identifies what the notice is about now, so dismissing it applies to that alone. */
  private get noticeKey(): string | null {
    const current = this.state;

    switch (current.kind) {
      case 'available':
      case 'downloading':
      case 'installing':
      case 'readyToRelaunch':
        return `${current.kind}:${current.info.version}`;

      case 'failed':
        // A failed quiet check is for Settings only; a failed install happened because the user (or their policy) asked for it.
        return current.during === 'install' && current.info ? `failed:${current.info.version}` : null;

      default:
        return null;
    }
  }

  get noticeVisible(): boolean {
    const key = this.noticeKey;

    return key !== null && !this.dismissed.includes(key);
  }

  /** Whether the app should show an update indicator at all (the notice may have been sent away). */
  get pending(): boolean {
    return this.noticeKey !== null;
  }

  /** "Later": hides the notice for this version and stage. The status bar keeps a quiet indicator. */
  dismiss(): void {
    const key = this.noticeKey;
    if (key) {
      this.dismissed = [...this.dismissed, key];
    }
  }

  /** Asks the release manifest whether a newer version exists. */
  async check(): Promise<void> {
    if (this.busy) {
      return;
    }

    this.state = { kind: 'checking' };

    try {
      const info = await this.transport.invoke('update_check', undefined);
      this.state = info ? { kind: 'available', info } : { kind: 'upToDate', checkedAt: new Date() };
    } catch (error) {
      this.state = this.failure(error, 'check', null);
    }
  }

  /** Downloads and installs the update found by the last check; progress arrives in `state`. */
  async install(): Promise<void> {
    const info = this.installable();
    if (!info) {
      return;
    }

    this.state = { kind: 'downloading', info, progress: null };

    try {
      await this.transport.invoke('update_install', { onProgress: (progress) => this.advance(info, progress) });
      this.state = { kind: 'readyToRelaunch', info };
    } catch (error) {
      this.state = this.failure(error, 'install', info);
    }
  }

  /** The restart a user asks for: straight away, or after a confirmation when it would stop chats that are working. */
  async requestRelaunch(): Promise<void> {
    const working = this.environment.workingChats();

    if (working > 0) {
      this.chatsAtRisk = working;
      this.confirmingRestart = true;

      return;
    }

    await this.relaunch();
  }

  async relaunch(): Promise<void> {
    try {
      await this.transport.invoke('app_relaunch', undefined);
    } catch (error) {
      this.notifications.push({ title: t('updates.restart.failed'), description: IpcFailure.from(error).message, tone: 'danger' });
    }
  }

  /**
   * The startup policy, applied once the shell is showing (not during the intro or setup).
   * Only a found update ever speaks up: "not configured" and network trouble stay in Settings.
   */
  async startup(): Promise<void> {
    if (this.started) {
      return;
    }

    this.started = true;

    const policy = this.preferences.value?.updatePolicy ?? 'startupAsk';
    if (policy === 'off') {
      return;
    }

    this.lastLook = Date.now();
    await this.check();
    await this.autoInstall(policy);
  }

  /**
   * Looks again while the app stays open, every `RECHECK_MS` and when the window comes back to the
   * front, so a release made after launch is found without a restart. Returns the stop function.
   */
  watch(): () => void {
    const look = () => {
      if (!document.hidden) {
        void this.recheck();
      }
    };
    const timer = setInterval(look, RECHECK_MS);
    document.addEventListener('visibilitychange', look);
    window.addEventListener('focus', look);

    return () => {
      clearInterval(timer);
      document.removeEventListener('visibilitychange', look);
      window.removeEventListener('focus', look);
    };
  }

  /** A quiet look for a newer release: no "checking" state, a failure keeps what is shown, and it never disturbs an update in progress. */
  private async recheck(): Promise<void> {
    const policy = this.preferences.value?.updatePolicy ?? 'startupAsk';
    const state = this.state;
    const current = state.kind;
    const idle = current === 'idle' || current === 'upToDate' || (state.kind === 'failed' && state.during === 'check');
    if (policy === 'off' || !this.started || this.rechecking || !idle || Date.now() - this.lastLook < MIN_GAP_MS) {
      return;
    }

    this.rechecking = true;
    this.lastLook = Date.now();

    try {
      const info = await this.transport.invoke('update_check', undefined);
      if (this.state.kind === current) {
        this.state = info ? { kind: 'available', info } : { kind: 'upToDate', checkedAt: new Date() };
      }
    } catch {
      // Quiet: the next look tries again; Settings shows failures of a check the user asked for.
    } finally {
      this.rechecking = false;
    }

    await this.autoInstall(policy);
  }

  private async autoInstall(policy: string): Promise<void> {
    // An installer that closes the app must not do so under a running agent: ask instead.
    const unattended = !(this.environment.installerClosesApp() && this.environment.agentsRunning());
    if (policy === 'autoInstall' && this.state.kind === 'available' && unattended) {
      await this.install();
    }
  }

  /** The update an install may start from: a found one, or one whose earlier install failed in a way worth repeating. */
  private installable(): UpdateInfo | null {
    const current = this.state;

    if (current.kind === 'available') {
      return current.info;
    }

    if (current.kind === 'failed' && current.during === 'install' && current.info && retryable(current.reason)) {
      return current.info;
    }

    return null;
  }

  private advance(info: UpdateInfo, progress: UpdateProgress): void {
    // Late events of a finished or failed install must not move the state back.
    if (this.state.kind !== 'downloading' && this.state.kind !== 'installing') {
      return;
    }

    if (progress.phase === 'downloading') {
      this.state = { kind: 'downloading', info, progress };
    } else if (progress.phase === 'installing') {
      this.state = { kind: 'installing', info };
    }
  }

  private failure(error: unknown, during: Failed['during'], info: UpdateInfo | null): UpdateState {
    const verdict = classify(error);

    return verdict.notConfigured ? { kind: 'notConfigured' } : { kind: 'failed', during, reason: verdict.reason, info };
  }
}

export const updates = new UpdatesStore(ipc, prefs, toasts, {
  installerClosesApp: () => app.info?.os === 'windows',
  agentsRunning: () => agentSessions.sessions.length > 0,
  workingChats: () => agentSessions.sessions.filter((session) => session.streaming).length,
});
