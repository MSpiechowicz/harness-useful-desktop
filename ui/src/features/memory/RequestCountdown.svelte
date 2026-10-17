<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { APPROVAL_WINDOW_MS } from '$lib/stores/scriptRequests.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** When the server stops waiting for the answer, in epoch milliseconds. */
    deadline: number;
    /** The time now. The dialog keeps it ticking, so this and the buttons it disables agree. */
    now: number;
  }

  let { deadline, now }: Props = $props();

  /** Below this many seconds the countdown asks for attention. */
  const URGENT_SECONDS = 10;

  const left = $derived(Math.min(APPROVAL_WINDOW_MS, Math.max(0, deadline - now)));
  const seconds = $derived(Math.ceil(left / 1000));
  const urgent = $derived(seconds <= URGENT_SECONDS);
</script>

<div class="flex items-center gap-3">
  <span class={cx('tabular min-w-[8ch] text-sm whitespace-nowrap transition-colors duration-[var(--dur-base)]', urgent ? 'text-warning' : 'text-fg-2')}>{t('memory.request.secondsLeft', { count: seconds })}</span>
  <div
    role="progressbar"
    aria-label={t('memory.request.timeLeft')}
    aria-valuemin={0}
    aria-valuemax={APPROVAL_WINDOW_MS / 1000}
    aria-valuenow={seconds}
    class="h-1 w-24 overflow-hidden rounded-full bg-surface-4"
  >
    <div class={cx('h-full rounded-full transition-[width,background-color] duration-300 ease-linear', urgent ? 'bg-warning' : 'bg-accent')} style:width="{(left / APPROVAL_WINDOW_MS) * 100}%"></div>
  </div>
</div>
