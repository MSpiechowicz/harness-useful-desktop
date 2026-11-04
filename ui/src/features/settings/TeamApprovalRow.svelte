<script lang="ts">
  import Lock from '@lucide/svelte/icons/lock';
  import { tick, untrack } from 'svelte';

  import Switch from '$lib/components/Switch.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
  import { knowledgeStats } from '$lib/stores/knowledgeStats.svelte';
  import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
  import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
  import { motion } from '$lib/stores/motion.svelte';
  import { formatDateTime } from '$lib/utils/format';

  import { approvalFocus } from './approval-focus.svelte';
  import SettingRow from './SettingRow.svelte';
  import { approvalBlock, approvalWhy, changedLine } from './team-approval';
  import { teamApproval } from './team-approval.svelte';

  interface Props {
    /** A team database is saved. Without one there is nowhere to keep the setting. */
    remoteSaved: boolean;
  }

  let { remoteSaved }: Props = $props();

  /** How long the ring that points at the row stays. */
  const FLASH_MS = 1600;
  /** How long a smooth scroll to the row may take before the ring comes anyway. */
  const SCROLL_SETTLE_MS = 1200;
  /** How often the scroll is started again while the page is still growing under it. */
  const SCROLL_TRIES = 3;

  let row: HTMLElement | undefined = $state();
  let flashing = $state(false);

  const current = $derived(teamApproval.value);
  // A server that refused to answer says what the feature list could not: it predates team approval.
  const supported = $derived(knowledgeStatus.value?.features.docsApproval ?? (teamApproval.readFailure === 'unsupported' ? false : null));
  const block = $derived(
    approvalBlock({
      remoteSaved,
      supported,
      unreachable: knowledgeChanges.pulse?.offline === true || teamApproval.readFailure === 'unreachable',
      writable: current?.writable ?? null,
      noName: knowledgeViewer.noName,
    }),
  );
  /** Who switched it last, in a grey line under the note: the exact moment is in its tooltip. */
  const changed = $derived(changedLine(current, (name) => knowledgeViewer.is(name)));

  // The setting is read when the row opens and again whenever the database says it changed (a teammate switched it): no Refresh.
  $effect(() => {
    void knowledgeChanges.pulse?.approval;
    void remoteSaved;

    if (remoteSaved) {
      // The stats say who the user is, so a missing git name is said up front.
      untrack(() => void Promise.all([teamApproval.load(), knowledgeStats.load()]));
    }
  });

  /** Rings the row for a moment. */
  function flash(): void {
    flashing = true;
    setTimeout(() => (flashing = false), FLASH_MS);
  }

  function inView(target: HTMLElement): boolean {
    const box = target.getBoundingClientRect();

    return box.top >= 0 && box.bottom <= window.innerHeight;
  }

  /** Waits for the scroll that was just started to end, or a moment when the browser does not say so. */
  function scrollSettled(): Promise<void> {
    return new Promise((resolve) => {
      const finish = (): void => {
        document.removeEventListener('scrollend', finish, true);
        clearTimeout(timer);
        resolve();
      };
      const timer = setTimeout(finish, SCROLL_SETTLE_MS);

      document.addEventListener('scrollend', finish, true);
    });
  }

  /** Scrolls to the row and rings it. The section may still be opening when the link is followed (its panel slides in, the page grows), so a row that is out of view when the scroll ends is scrolled to again. */
  async function reveal(target: HTMLElement): Promise<void> {
    for (let attempt = 0; attempt < SCROLL_TRIES && !inView(target); attempt += 1) {
      target.scrollIntoView({ behavior: motion.level === 'full' ? 'smooth' : 'auto', block: 'center' });
      await scrollSettled();
    }

    flash();
  }

  // A link from the import dialog asks to see this row: scroll to it (when it is not in view already) and ring it for a moment.
  $effect(() => {
    if (!approvalFocus.requested || !row) {
      return;
    }

    const target = row;
    approvalFocus.take();

    void tick().then(() => reveal(target));
  });
</script>

<!-- The note and the reason hang under the whole row, not under the switch: both are full sentences. -->
<div bind:this={row} data-flash={flashing ? '' : undefined} class="relative rounded-md">
  <SettingRow title={t('settings.knowledgeApproval.title')} description={t('settings.knowledgeApproval.description')}>
    {#snippet control()}
      <Switch
        label={t('settings.knowledgeApproval.title')}
        disabled={block !== null || current === null || teamApproval.pending}
        bind:checked={() => current?.approval ?? false, (next) => void teamApproval.set(next)}
      />
    {/snippet}
  </SettingRow>

  <div class="flex flex-col gap-2 pb-4">
    <p class="text-sm text-fg-3">{t('settings.knowledgeApproval.note')}</p>
    {#if changed !== null}
      <p class="text-sm text-fg-3" title={current?.changedAt ? formatDateTime(current.changedAt) : undefined}>{changed}</p>
    {/if}
    {#if block !== null}
      <p class="flex items-center gap-2 text-sm text-fg-3" role="status"><Lock size={ICON.sm} class="shrink-0" aria-hidden="true" />{t(approvalWhy(block))}</p>
    {/if}
  </div>
</div>
