import { t } from '$lib/i18n/i18n.svelte';
import type { AppPrefs } from '$lib/ipc/bindings/AppPrefs';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { memoryChat } from '$lib/stores/memoryChat.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts, type ToastsStore } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

/** What decides whether the notice is due. A fact that is not known yet is null. */
export interface OptInFacts {
  /** The notice was handled before. */
  done: boolean | null;
  /** Project memory is turned on. Without it the Settings cards are greyed out, so the notice waits. */
  enabled: boolean | null;
  /** Harness Memory is installed on this computer. */
  installed: boolean | null;
  /** The installed Harness Memory applies the opt-in guards (an older one cannot be told where to save). */
  supported: boolean | null;
}

interface Dependencies {
  transport: Transport;
  notifications: Pick<ToastsStore, 'push'>;
  /** Read synchronously, so an effect that calls `run` follows every fact. */
  facts: () => OptInFacts;
  /** The stored preferences after the notice was marked done. */
  saved: (stored: AppPrefs) => void;
  openSettings: () => void;
}

/**
 * Tells an existing user once that agents save notes and scripts only where they turn it on. A user without Harness Memory has nothing
 * to learn and is marked done silently. One whose Harness Memory is too old to apply the opt-in hears it once it is updated, and one with
 * Project memory off once it is on again. It is
 * marked done as it is shown, never on dismissal, so it never repeats. A mark that fails is logged and nothing is shown.
 */
export class MemoryOptInNotice {
  private asked = false;

  constructor(private readonly deps: Dependencies) {}

  /** Never throws. Call it where the facts are tracked: it does nothing until they are known, and once it asked, nothing again. */
  async run(): Promise<void> {
    if (this.asked) {
      return;
    }

    const facts = this.deps.facts();

    if (facts.done !== false || facts.installed === null || (facts.installed && facts.supported === null)) {
      return;
    }

    if (facts.installed && (!facts.supported || facts.enabled !== true)) {
      return;
    }

    this.asked = true;

    try {
      this.deps.saved(await this.deps.transport.invoke('memory_opt_in_notice_done', undefined));
    } catch (error) {
      console.warn('could not mark the memory opt-in notice as done', error);

      return;
    }

    if (facts.installed) {
      this.announce();
    }
  }

  private announce(): void {
    this.deps.notifications.push({
      title: t('memory.guards.optIn.title'),
      description: t('memory.guards.optIn.body'),
      duration: 0,
      action: { label: t('memory.guards.optIn.action'), run: this.deps.openSettings },
    });
  }
}

export const memoryOptInNotice = new MemoryOptInNotice({
  transport: ipc,
  notifications: toasts,
  facts: () => ({
    done: prefs.value?.memoryOptInNoticeDone ?? null,
    enabled: prefs.value ? prefs.value.memoryEnabled !== false : null,
    installed: ecosystem.status ? ecosystem.status.items.some((item) => item.id === 'memory' && item.installed) : null,
    supported: memoryRemote.value ? memoryChat.supported : null,
  }),
  saved: (stored) => prefs.keepMemoryOptInNoticeDone(stored.memoryOptInNoticeDone),
  openSettings: () => {
    viewState.settingsTarget = 'memory';
    nav.go('settings');
  },
});
